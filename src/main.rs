use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::time;
use tide_websockets::WebSocket;
use async_std::stream::StreamExt;
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
}

impl State {
    fn lock_repo(&self) -> Result<MutexGuard<repository::Repo>, tide::Error> {
        tide::log::trace!("Locking repo...");
        return self.repo.lock().map_err(|e|tide::Error::from_str(500,format!("Couldn't lock database: {:?}",e)));
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

                let user_id: u32 = repo.get_authenticated_user_id(&cred)?
                    .ok_or(tide::Error::from_str(401, "Incorrect username or password"))?;

                return Ok((user_id, username));
            },
            _ => { return Err(tide::Error::from_str(400, "Authroization type is unknown")) }
        }
    }
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    tide::log::with_level(tide::log::LevelFilter::Info);

    let repo = repository::Repo::new("messages.db")
        .expect("Error while initializing database");
    let mut tera = tera::Tera::new("templates/*.html")
        .expect("Could not load templates");
    tera.autoescape_on(vec!["html", ".sql"]);

    let mut app = tide::with_state( State {
        repo: Arc::new(Mutex::new(repo)),
        view: Arc::new(view::View { tera }),
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
        let messages = repo.select_messages()?;
        let users = repo.select_users()?;
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

        // parse message
        tide::log::trace!("Parsing message request as FORM..");
        let body: std::collections::HashMap<String,String> = req.body_form().await?;
        let repo = req.state().lock_repo()?;
        let users = repo.select_users()?;
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
        
        // Return fresh page
        tide::log::trace!("Selecting fresh messages...");
        let messages = repo.select_messages()?;
        tide::log::trace!("Rendering fresh page...");
        let body = req.state().view.render_index(messages, users)
            .map_err(|e| tide::Error::new(500, e))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());
    });

    app.listen("0.0.0.0:8080").await?;
    Ok(())
}




