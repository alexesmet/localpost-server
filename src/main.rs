use std::sync::{Arc, Mutex, MutexGuard};
use tide::prelude::json;
use tide::Request;
use ascii;
use blake3;
use tera;

mod view;
mod model;
mod repository;

#[derive(Clone)]
struct State {
    repo: Arc<Mutex<repository::Repo>>,
    view: Arc<view::View>
}

impl State {
    fn lock_repo(&self) -> Result<MutexGuard<repository::Repo>, tide::Error> {
        return self.repo.lock().map_err(|e| tide::Error::from_str(500, format!("Could not lock database: {:?}",e)));
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
        view: Arc::new(view::View { tera })
    });

    app.at("/static").serve_dir("templates/static")?;


    // web pages
    app.at("/").get(|req: Request<State>| async move {
        let cred = match get_credentials(&req) {
            Ok(token) => token,
            Err(_) => {
                return Ok(tide::Response::builder(401)
                    .header("WWW-Authenticate", "Basic")
                    .build())
            }
        };
        let repo = req.state().lock_repo()?;
        let user_id: u32 = match repo.get_authenticated_user_id(&cred)? {
            Some(n) => { n }
            None => { repo.register_user(&cred)?
                .ok_or(tide::Error::from_str(401, "Incorrect username or password"))? }
        };

        let messages = repo.select_messages_for_user(user_id)?;
        let users = repo.select_users_all()?;

        let body = req.state().view.render_index(messages, users)
            .map_err(|e| tide::Error::new(500, e))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());
    });

    
    app.at("/").post(|mut req: Request<State>| async move {
        let cred = get_credentials(&req)?;
        let body: std::collections::HashMap<String,String> = req.body_form().await?;

        let repo = req.state().lock_repo()?;
        let user_id = repo.get_authenticated_user_id(&cred)?
            .ok_or(tide::Error::from_str(401, "Incorrect username or password"))?;
        let users = repo.select_users_all()?;



        let text = body.get("text").ok_or(tide::Error::from_str(400, "Missing text field"))?;
        let recipients: Vec<u32> = users.iter()
            .map(|u| (u.id, format!("usr{}", u.id)))
            .map(|i| (i.0, body.get(&i.1).is_some()))
            .filter(|i| i.1)
            .map(|i| i.0)
            .collect();

        if recipients.len() == 0 {
            return Ok(tide::Response::builder(400).body("No message recipients").build());
        }

        let message = model::PostMessageRequest { recipients, text: text.to_string() };

        repo.insert_message(user_id, message)?;


        return Ok(tide::Response::new(200));
    });

    
    app.at("/messages").get(|req: Request<State>| async move {
        let cred = get_credentials(&req)?;
        let repo = req.state().lock_repo()?;
        let user_id = repo.get_authenticated_user_id(&cred)?
            .ok_or(tide::Error::from_str(401, "Incorrect username or password"))?;
        let messages = repo.select_messages_for_user(user_id)?;

        return Ok(json!(messages));

    });

    // Sending messages
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

    let mut token_split = ascii::AsciiStr::from_ascii(&token_bytes)
        .map_err(|_| tide::Error::from_str(500, "Having a hard time decoding base64 to ascii"))?
        .split(ascii::AsciiChar::Colon);
    
    let username = token_split.next()
        .ok_or(tide::Error::from_str(400,"Incorrect token format"))
        .map(ascii::AsciiStr::to_string)?;

    let password = token_split.next()
        .ok_or(tide::Error::from_str(400,"Incorrect token format"))
        .map(ascii::AsciiStr::as_bytes)
        .map(|v| blake3::hash(v).to_hex().to_string())?;

    return Ok( model::UserCredentials { username, password });
}
