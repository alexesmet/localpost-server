use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::time;
use tide_websockets::WebSocket;
use async_std::stream::StreamExt;
use tide::prelude::json;
use tide::Request;
use std::iter::Iterator;

use futures::io::{AsyncBufRead, AsyncBufReadExt};

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
        tide::log::trace!("Locking repo...");
        return self.repo.lock().map_err(|e|tide::Error::from_str(500,format!("Couldn't lock database: {:?}",e)));
    }

    fn broadcast_message(&self, msg: &model::MessageResponse) -> Result<(), tide::Error> {
        tide::log::debug!("Locking message listeners for notification...");
        self.messages_txs.lock()
            .map_err(|e| tide::Error::from_str(500, format!("Could not lock websockets: {:?}",e)))?
            .retain(|(id, tx)| {
                if !msg.sender_id.eq(id) && !msg.recipients.iter().any(|r| r.id.eq(id)) {
                    return true;
                }
                let sending_result = tx.send(msg.clone());
                if let Err(mpsc::SendError(_)) = sending_result { 
                    tide::log::debug!("Removing one message listener");
                    return false;
                } else { return true; }
            });
        tide::log::debug!("Releasing message listeners for notification...");
        return Ok(());
    }
    fn create_token(username: String, user_id: u32, exp_time: u64) -> String {
        let token_a = base64::encode(format!("{}:{}:{}", username, user_id, exp_time));
        let token_a_salt = format!("{}{}", token_a, SERVER_SECRET);
        let token_b = blake3::hash(token_a_salt.as_bytes()).to_hex().to_string();
        return format!("{}.{}", token_a, token_b);
    }

    fn parse_token(token: &str) -> Option<(String,u32)> {
        let mut token_split = token.split('.');
        let token_a = token_split.next()?;
        let token_b = token_split.next()?;
        let token_a_salt = format!("{}{}", token_a, SERVER_SECRET);

        // TODO: Check if password changed!!!
        if blake3::hash(token_a_salt.as_bytes()).to_hex().to_string().ne(token_b) {
            tide::log::warn!("Token {} has incorrect hash.", token_a);
            return None;
        } else {
            let decoded = String::from_utf8(base64::decode(token_a).ok()?).ok()?;
            let mut split = decoded.split(':');
            let username = split.next()?;
            let user_id = split.next()?;
            let exp_time = split.next()?;

            let now = time::SystemTime::now().duration_since(time::UNIX_EPOCH).ok()?;
            if time::Duration::new(exp_time.parse().ok()?, 0) < now { 
                tide::log::warn!("Token {} expeired.", token_a);
                return None;
            }

            return Some((username.to_string(), user_id.parse().ok()?));
        }
    }

    fn get_authenticated_user_id(&self, req: &Request<State>) -> Result<(u32, String), tide::Error> {
        let mut authorization_words = req.header("Authorization")
            .ok_or(tide::Error::from_str(401, "Authorization token is not provided"))?
            .as_str()
            .split_whitespace();

        let auth_type = authorization_words.next()
            .ok_or(tide::Error::from_str(400, "Unexpected end of Authorization token"))?;

        let repo = self.lock_repo()?;
        tide::log::trace!("AUTH: Repo locked.");

        match auth_type {
            "Bearer" => {
                let token_encoded = authorization_words.next()
                    .ok_or(tide::Error::from_str(400, "Unexpected end of Authorization token"))?;

                let (username, id) = Self::parse_token(token_encoded)
                    .ok_or(tide::Error::from_str(401, "Bearer token is invalid"))?;

                return Ok((id, username));
            },
            "Basic" => {
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

                let cred = model::UserCredentials { username: username.clone(), password };

                let user_id: u32 = match repo.get_authenticated_user_id(&cred)? {
                    Some(n) => { n }
                    None => { repo.register_user(&cred)?
                        .ok_or(tide::Error::from_str(401, "Incorrect username or password"))? }
                };

                return Ok((user_id, username));
            },
            _ => { return Err(tide::Error::from_str(400, "Authroization type is unknown")) }
        }
    }
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    //tide::log::with_level(tide::log::LevelFilter::Info);

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
        // authenticate with credentials
        let (user_id, username) = match req.state().get_authenticated_user_id(&req) {
            Ok(ok) => { ok }
            Err(e) => { return Ok(tide::Response::builder(401)
                            .header("WWW-Authenticate", "Basic")
                            .body(e.to_string())
                            .build()); }
        };
        // generate authorization token
        let expiration_time = (time::SystemTime::now()+TOKEN_EXPIRATION)
            .duration_since(time::UNIX_EPOCH)
            .expect("Can not count time anymore")
            .as_secs();
        let token = State::create_token(username, user_id, expiration_time);

        // render page
        let repo = req.state().lock_repo()?;
        let messages = repo.select_messages_for_user(user_id)?;
        let users = repo.select_users_all()?;
        let body = req.state().view.render_index(messages, users)
            .map_err(|e| tide::Error::new(500, e))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .header("Set-Cookie", format!("token={}; Max-Age={}", token, TOKEN_EXPIRATION.as_secs()))
            .build())
    });

    // html form 
    app.at("/").post(|mut req: Request<State>| async move {
        // auth
        let (user_id, _) = req.state().get_authenticated_user_id(&req)?;

        // get file
        tide::log::trace!("Reading content-type from the request...");
        let content_type = req.header("Content-Type")
            .ok_or(tide::Error::from_str(400, "Content-Type is not provided"))?
            .as_str();

        let mut content_type_split = content_type.split(";");
        let content_type_type = content_type_split.next()
            .ok_or(tide::Error::from_str(400, "Content-Type is not provided"))?;

        if content_type_type == "multipart/form-data" {


            let mut content_type_boundary = content_type_split.next()
                .ok_or(tide::Error::from_str(400, "Boundary is not provided (A)"))?
                .trim()
                .split("=");

            if content_type_boundary.next()
                .ok_or(tide::Error::from_str(400,"Boundary is not provided (B)"))? != "boundary" {
                return Err(tide::Error::from_str(400, "Boundary is not provided (C)"));
            }

            let boundary = content_type_boundary.next()
                .ok_or(tide::Error::from_str(400,"Boundary is not provided (D)"))?;

            tide::log::trace!("Reading file from the request...");
            let mut reader = req.take_body().into_reader();
            loop {
                let buf: &[u8] = reader.fill_buf().await?;
                println!("%---");
                println!("{:?}", std::str::from_utf8(buf));
                let len = buf.len();
                if len < 1 { break; }
                reader.consume_unpin(len);

            }
            todo!();


        } else {
            // parse message
            tide::log::trace!("Parsing message request as FORM..");
            let body: std::collections::HashMap<String,String> = req.body_form().await?;
            let repo = req.state().lock_repo()?;
            let users = repo.select_users_all()?;
            tide::log::trace!("Parsing message request..");
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
            tide::log::trace!("Inserting message to DB..");
            // insert message
            let message = model::PostMessageRequest { recipients, text: text.to_string() };
            let response = repo.insert_message(user_id, message)?;


            // Send to websockets
            tide::log::trace!("Broadcast message to listeners...");
            req.state().broadcast_message(&response)?;
            
            // Return fresh page
            tide::log::trace!("Selecting fresh messages...");
            let messages = repo.select_messages_for_user(user_id)?;
            tide::log::trace!("Rendering fresh page...");
            let body = req.state().view.render_index(messages, users)
                .map_err(|e| tide::Error::new(500, e))?;

            return Ok(tide::Response::builder(200)
                .body(body)
                .content_type(tide::http::mime::HTML)
                .build());
        }
    });

    // websocket
    app.at("/websocket").get(WebSocket::new(|req: Request<State>, mut stream| async move {
        use std::borrow::Cow;

        tide::log::trace!("Websocket: Reading first msg from websocket");
        let first_msg = stream.next().await
            .ok_or(tide_websockets::Error::Protocol(Cow::from("Unexpected end of stream")))??;

        tide::log::trace!("Websockets: Received token");
        let token: String = if let tide_websockets::Message::Text(s) = first_msg { Ok(s) } else {
            Err(tide_websockets::Error::Protocol(Cow::from("Unexpected end of stream")))
        }?;

        tide::log::trace!("Websockets: Parsing token...");
        let (_, user_id) = State::parse_token(&token)
            .ok_or(tide_websockets::Error::Protocol(Cow::from("Could not parse token")))?;

        tide::log::trace!("Websockets: Token parsed.");
        let (tx, rx): (mpsc::Sender<_>, mpsc::Receiver<_>) = mpsc::channel();
        tide::log::trace!("Websockets: Pushing listener to State...");
        { req.state()
            .messages_txs
            .lock()
            .map_err(|_| tide_websockets::Error::Protocol(Cow::from("Could not lock state")))?
            .push( (user_id,tx) ); }
        
        tide::log::trace!("Websockets: Entering loop...");
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
        // auth
        let (user_id, _) = req.state().get_authenticated_user_id(&req)?;
        // lock
        let repo = req.state().lock_repo()?;
        // get
        let messages = repo.select_messages_for_user(user_id)?;

        return Ok(json!(messages));

    });

    // REST post message. TODO: Two-way auth when https is ready
    app.at("/messages").post(|mut req: Request<State>| async move {
        let (user_id, _) = req.state().get_authenticated_user_id(&req)?;

        let body: model::PostMessageRequest = req.body_json().await?;
        let repo = req.state().lock_repo()?;
        let response = repo.insert_message(user_id, body)?;

        req.state().broadcast_message(&response)?;

        return Ok(tide::Response::builder(201)
            .body(json!(response))
            .build());
    });

    app.listen("0.0.0.0:8080").await?;
    Ok(())
}




