use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use async_std::prelude::StreamExt;
use std::time;
use tide_websockets::WebSocket;
use tide::prelude::json;
use tide::Request;
use std::iter::Iterator;
use blake3;
use tera;
use base64;

mod view;
mod model;
mod repository;

const SERVER_SECRET: &str = "KOAfpNcYBnmqEi9Yxg9335bP0nWOL3I5upuSXVkiLw4";
const TOKEN_EXPIRATION: time::Duration = time::Duration::from_secs(86400);

#[derive(Clone)]
struct State {
    repo: Arc<Mutex<repository::Repo>>,
    view: Arc<view::View>, 
    messages_txs: Arc<Mutex<Vec<(u32, mpsc::Sender<model::MessageResponse>)>>>
}

impl State {
    fn lock_repo(&self) -> Result<MutexGuard<repository::Repo>, tide::Error> {
        return self.repo.lock().map_err(|e| tide::Error::from_str(500,format!("Could not lock database: {:?}",e)));
    }
}


#[async_std::main]
async fn main() -> tide::Result<()> {
    tide::log::start();

    let repo = repository::Repo::new("messages.db")
        .expect("Error while initializing database");
    let mut tera = tera::Tera::new("templates/*.html")
        .expect("Could not load templates");
    tera.autoescape_on(vec!["html", ".sql"]);


    let mut app = tide::with_state( State {
        repo: Arc::new(Mutex::new(repo)),
        view: Arc::new(view::View { tera }),
        messages_txs: Arc::new(Mutex::new(Vec::new()))
    });
    app.with(tide_compress::CompressMiddleware::new());


    app.at("/static").serve_dir("templates/static")?;



    // web pages
    app.at("/").get(|req: Request<State>| async move {
        // get credentials
        let cred = match get_credentials(&req) {
            Ok(token) => token,
            Err(_) => {
                return Ok(tide::Response::builder(401)
                    .header("WWW-Authenticate", "Basic")
                    .build())
            }
        };
        // authenticate with credentials
        let repo = req.state().lock_repo()?;
        let user_id: u32 = match repo.get_authenticated_user_id(&cred)? {
            Some(n) => { n }
            None => { repo.register_user(&cred)?
                .ok_or(tide::Error::from_str(401, "Incorrect username or password"))? }
        };
        // generate authorization token
        let expiration_time = (time::SystemTime::now()+TOKEN_EXPIRATION)
            .duration_since(time::UNIX_EPOCH)
            .expect("Can not count time anymore")
            .as_secs();
        
        let token = create_token(cred.username, user_id, expiration_time);

        // render page
        let messages = repo.select_messages_for_user(user_id)?;
        let users = repo.select_users_all()?;
        let body = req.state().view.render_index(messages, users)
            .map_err(|e| tide::Error::new(500, e))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .header("Set-Cookie", format!("token={}; Max-Age={}", token, TOKEN_EXPIRATION.as_secs()))
            .build());
    });

    // html form 
    app.at("/").post(|mut req: Request<State>| async move {
        // get credentials
        let cred = get_credentials(&req)?;
        let body: std::collections::HashMap<String,String> = req.body_form().await?;
        // authenticate with credentials
        let repo = req.state().lock_repo()?;
        let user_id = repo.get_authenticated_user_id(&cred)?
            .ok_or(tide::Error::from_str(401, "Incorrect username or password"))?;
        let users = repo.select_users_all()?;
        // parse message
        let text = body.get("text").ok_or(tide::Error::from_str(400, "Missing text field"))?;
        let recipients: Vec<u32> = users.iter()
            .map(|u| (u.id, format!("usr{}", u.id)))
            .map(|i| (i.0, body.get(&i.1).is_some()))
            .filter(|i| i.1)
            .map(|i| i.0)
            .collect();
        // validate message
        if recipients.len() == 0 {
            return Ok(tide::Response::builder(400)
                .body(format!("No message recipients provided. Your message: {}", text))
                .build());
        }
        let message = model::PostMessageRequest { recipients, text: text.to_string() };
        let response = repo.insert_message(user_id, message)?;


        // Send to websockets
        broadcast_message(&(req.state().messages_txs), &response)?;
        
        // Return fresh page
        let messages = repo.select_messages_for_user(user_id)?;
        let body = req.state().view.render_index(messages, users)
            .map_err(|e| tide::Error::new(500, e))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());

    });

    // websocket: WIP
    app.at("/websocket").get(WebSocket::new(|req: Request<State>, mut stream| async move {
        use std::borrow::Cow;

        let first_msg = stream.next().await
            .ok_or(tide_websockets::Error::Protocol(Cow::from("Unexpected end of stream")))??;


        let token: String = if let tide_websockets::Message::Text(s) = first_msg { Ok(s) } else {
            Err(tide_websockets::Error::Protocol(Cow::from("Unexpected end of stream")))
        }?;

        let (_, user_id) = parse_token(token)
            .ok_or(tide_websockets::Error::Protocol(Cow::from("Could not parse token")))?;

        let (tx, rx): (mpsc::Sender<_>, mpsc::Receiver<_>) = mpsc::channel();
        { req.state()
            .messages_txs
            .lock()
            .map_err(|_| tide_websockets::Error::Protocol(Cow::from("Could not lock state")))?
            .push( (user_id,tx) ); }
        
        loop {
            let rcv = rx.recv();
            if let Ok(msg) = rcv {
                if let Err(_) = stream.send_json(&msg).await {
                    break;
                }
            } else { break; }
        }
        return Ok(());
    }));

    // WIP
    app.at("/messages").get(|req: Request<State>| async move {
        let cred = get_credentials(&req)?;
        let repo = req.state().lock_repo()?;
        let user_id = repo.get_authenticated_user_id(&cred)?
            .ok_or(tide::Error::from_str(401, "Incorrect username or password"))?;
        let messages = repo.select_messages_for_user(user_id)?;

        return Ok(json!(messages));

    });

    // WIP
    app.at("/messages").post(|mut req: Request<State>| async move {
        let cred = get_credentials(&req)?;
        let body: model::PostMessageRequest = req.body_json().await?;
        let repo = req.state().lock_repo()?;
        let user_id = repo.get_authenticated_user_id(&cred)?
            .ok_or(tide::Error::from_str(401, "Incorrect username or password"))?;
        let response = repo.insert_message(user_id, body)?;
        return Ok(tide::Response::builder(201)
            .body(json!(response))
            .build());
    });

    app.listen("0.0.0.0:8080").await?;
    Ok(())
}

fn create_token(username: String, user_id: u32, exp_time: u64) -> String {
    let token_a = base64::encode(format!("{}:{}:{}", username, user_id, exp_time));
    let token_a_salt = format!("{}{}", token_a, SERVER_SECRET);
    let token_b = blake3::hash(token_a_salt.as_bytes()).to_hex().to_string();
    return format!("{}.{}", token_a, token_b);
}

fn parse_token(token: String) -> Option<(String,u32)> {
    let mut token_split = token.split('.');
    let token_a = token_split.next()?;
    let token_b = token_split.next()?;
    let token_a_salt = format!("{}{}", token_a, SERVER_SECRET);

    if blake3::hash(token_a_salt.as_bytes()).to_hex().to_string().ne(token_b) {
        return None;
    } else {
        let decoded = String::from_utf8(base64::decode(token_a).ok()?).ok()?;
        let mut split = decoded.split(':');
        let username = split.next()?;
        let user_id = split.next()?;
        let exp_time = split.next()?;

        let now = time::SystemTime::now().duration_since(time::UNIX_EPOCH).ok()?;
        if time::Duration::new(exp_time.parse().ok()?, 0) < now { return None; }

        return Some((username.to_string(), user_id.parse().ok()?));
    }
}





fn get_credentials(req: &Request<State>) -> Result<model::UserCredentials, tide::Error> {
    let mut authorization_words = req.header("Authorization")
        .ok_or(tide::Error::from_str(401, "Authorization token is not provided"))?
        .as_str()
        .split_whitespace();

    let authorization_is_basic = authorization_words.next()
        .ok_or(tide::Error::from_str(400, "Unexpected end of Authorization token"))?
        .eq("Basic");

    if !authorization_is_basic { 
        return Err(tide::Error::from_str(400, "Authroization type is not Basic"))
    }
    
    let token_encoded = authorization_words.next()
        .ok_or(tide::Error::from_str(400, "Unexpected end of Authorization token"))?;

    let token_bytes = base64::decode(token_encoded)
        .map_err(|_| tide::Error::from_str(400, "Incorrectly encoded basic token"))?;

    let token_string = String::from_utf8(token_bytes)
        .map_err(|_| tide::Error::from_str(500, "Having a hard time decoding base64 to ascii"))?;

    let mut token_split = token_string.split(":");
    
    let username = token_split.next()
        .ok_or(tide::Error::from_str(400,"Incorrect token format"))?
        .to_string();

    let password = token_split.next()
        .ok_or(tide::Error::from_str(400,"Incorrect token format"))
        .map(|v| v.as_bytes())
        .map(|v| blake3::hash(&v).to_hex().to_string())?;

    return Ok( model::UserCredentials { username, password });
}

fn broadcast_message(messages_txs: &Mutex<Vec<(u32, mpsc::Sender<model::MessageResponse>)>>, msg: &model::MessageResponse) -> Result<(), tide::Error> {
    messages_txs.lock()
        .map_err(|e| tide::Error::from_str(500, format!("Could not lock websockets: {:?}",e)))?
        .retain(|(id, tx)| {
            if !msg.sender_id.eq(id) && !msg.recipients.iter().any(|r| r.id.eq(id)) {
                return true;
            }
            let sending_result = tx.send(msg.clone());
            if let Err(mpsc::SendError(_)) = sending_result {
                return false;
            } else { 
                return true; 
            }
        });

    return Ok(());
}

