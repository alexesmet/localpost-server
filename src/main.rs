use std::sync::{Arc, Mutex, MutexGuard};
use tide::Request;
use std::iter::Iterator;


use tera;
use base64;

mod view;
mod model;
mod repository;

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

    fn get_authenticated_user(&self, req: &Request<State>) -> Result<model::UserResponse, tide::Error> {
        let mut authorization_words = req.header("Authorization")
            .ok_or(tide::Error::from_str(401, "Authorization token is not provided"))?
            .as_str()
            .split_whitespace();

        let auth_type = authorization_words.next()
            .ok_or(tide::Error::from_str(400, "Unexpected end of Authorization token"))?;

        let repo = self.lock_repo()?;
        tide::log::trace!("AUTH: Repo locked.");

        match auth_type {
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
                    .ok_or(tide::Error::from_str(400,"Incorrect token format"))?
                    .to_string();

                let cred = model::UserCredentials { username: username.clone(), password };

                let user = repo.get_authenticated_user(&cred)?
                    .ok_or(tide::Error::from_str(401, "Incorrect username or password"))?;

                return Ok(user);
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
        // guest page - no auth required

        // render page
        let repo = req.state().lock_repo()?;
        let messages = repo.select_messages()?;
        let body = req.state().view.render_index(messages, None)?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());
    });



    app.at("/login").get(|req: Request<State>| async move {
        // authenticate with credentials
        let user = match req.state().get_authenticated_user(&req) {
            Ok(ok) => { ok }
            Err(e) => { return Ok(tide::Response::builder(401)
                            .header("WWW-Authenticate", "Basic")
                            .body(e.to_string())
                            .build()); }
        };

        // render page
        let repo = req.state().lock_repo()?;
        let messages = repo.select_messages()?;
        let body = req.state().view.render_index(messages, Some(user))
            .map_err(|e| tide::Error::new(500, e))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());
    });

    // html form, for sending messages
    app.at("/login").post(|mut req: Request<State>| async move {
        // auth
        let user = req.state().get_authenticated_user(&req)?;

        // parse message
        let body: std::collections::HashMap<String,String> = req.body_form().await?;
        let repo = req.state().lock_repo()?;
        let text = body.get("text")
            .ok_or(tide::Error::from_str(400, "Missing text field"))?;

        // insert message
        let message = model::PostMessageRequest { text: text.to_string() };
        repo.insert_message(user.id, message)?;
        
        // Return fresh page
        let messages = repo.select_messages()?;
        let body = req.state().view.render_index(messages, Some(user))
            .map_err(|e| tide::Error::new(500, e))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());
    });

    // html form, for sending messages
    app.at("/admin").get(|req: Request<State>| async move {
        // auth
        let user = req.state().get_authenticated_user(&req)?;
        if ! user.admin {
            return Ok(tide::Response::new(tide::StatusCode::Unauthorized));
        }

        let repo = req.state().lock_repo()?;
        let users = repo.select_users()?;
        
        let body = req.state().view.render_admin(users)
            .map_err(|e| tide::Error::new(500, e))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());
    });

    app.at("/user/add").get(|req: Request<State>| async move {
        // auth
        let user = req.state().get_authenticated_user(&req)?;
        if ! user.admin {
            return Ok(tide::Response::new(tide::StatusCode::Unauthorized));
        }

        let body = req.state().view.render_user(None)?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());
    });

    app.at("/user/add").post(|mut req: Request<State>| async move {
        // auth
        let user = req.state().get_authenticated_user(&req)?;
        if ! user.admin {
            return Ok(tide::Response::new(tide::StatusCode::Unauthorized));
        }

        let body: std::collections::HashMap<String,String> = req.body_form().await?;
        let entity = model::UserResponse {
            id: 0,
            name: body.get("name")
                .ok_or(tide::Error::from_str(400, "Missing name field"))?
                .to_owned(),
            username: body.get("username")
                .ok_or(tide::Error::from_str(400, "Missing username field"))?
                .to_owned(),
            password: body.get("password")
                .ok_or(tide::Error::from_str(400, "Missing password field"))?
                .to_owned(),
            admin: body.get("admin") == Some(&"on".to_string()),

        };

        let repo = req.state().lock_repo()?;
        repo.create_user(entity)?;

        return Ok(tide::Response::builder(200)
            .body("User successfully created.")
            .content_type(tide::http::mime::HTML)
            .build());
    });



    app.at("/user/edit/:id").get(|req: Request<State>| async move {
        // auth
        let user = req.state().get_authenticated_user(&req)?;
        if ! user.admin {
            return Ok(tide::Response::new(tide::StatusCode::Unauthorized));
        }

        let id: u32 = req.param("id")?.parse()?;
        let repo = req.state().lock_repo()?;
        let select = repo.select_users()?;
        let user = select.iter()
            .find(|u| u.id == id)
            .ok_or(tide::Error::from_str(404, "NO SUCH USER"))?;

        let body = req.state().view.render_user(Some(user))?;

        return Ok(tide::Response::builder(200)
            .body(body)
            .content_type(tide::http::mime::HTML)
            .build());
    });

    app.at("/user/edit/:id").post(|mut req: Request<State>| async move {
        // auth
        let user = req.state().get_authenticated_user(&req)?;
        if ! user.admin {
            return Ok(tide::Response::new(tide::StatusCode::Unauthorized));
        }

        let body: std::collections::HashMap<String,String> = req.body_form().await?;
        let entity = model::UserResponse {
            id: req.param("id")?.parse()?,
            name: body.get("name")
                .ok_or(tide::Error::from_str(400, "Missing name field"))?
                .to_owned(),
            username: body.get("username")
                .ok_or(tide::Error::from_str(400, "Missing username field"))?
                .to_owned(),
            password: body.get("password")
                .ok_or(tide::Error::from_str(400, "Missing password field"))?
                .to_owned(),
            admin: body.get("admin") == Some(&"on".to_string()),

        };

        let repo = req.state().lock_repo()?;
        repo.update_user(entity)?;

        return Ok(tide::Response::builder(200)
            .body("User successfully updated.")
            .content_type(tide::http::mime::HTML)
            .build());
    });

    app.at("/user/delete/:id").get(|req: Request<State>| async move {
        // auth
        let user = req.state().get_authenticated_user(&req)?;
        if ! user.admin {
            return Ok(tide::Response::new(tide::StatusCode::Unauthorized));
        }

        let id: u32 = req.param("id")?.parse()?;

        let repo = req.state().lock_repo()?;
        repo.delete_user(id)?;

        return Ok(tide::Response::builder(200)
            .body("User successfully deleted.")
            .content_type(tide::http::mime::HTML)
            .build());
    });

    app.at("/delete/:id").get(|req: Request<State>| async move {
        // auth
        let user = req.state().get_authenticated_user(&req)?;
        if ! user.admin {
            return Ok(tide::Response::new(tide::StatusCode::Unauthorized));
        }

        let id: u32 = req.param("id")?.parse()?;

        let repo = req.state().lock_repo()?;
        repo.delete_message(id)?;

        return Ok(tide::Response::builder(200)
            .body("Message successfully deleted.")
            .content_type(tide::http::mime::HTML)
            .build());
    });



    app.listen("0.0.0.0:8080").await?;
    Ok(())
}




