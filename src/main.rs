use std::sync::{Arc, Mutex};
use tide::prelude::json;
use tide::Request;

mod model;
mod repository;

#[derive(Clone)]
struct State {
    repo: Arc<Mutex<repository::Repo>>
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    tide::log::start();

    let repo = repository::Repo::new("messages.db")
        .expect("Error while initializing database");
    let state = State { repo: Arc::new(Mutex::new(repo)) };
    let mut app = tide::with_state(state);

    app.at("/messages").get(|req: Request<State>| async move {

        let token = get_token(req)?;
        let repo = req.state().repo.lock() .expect("can not lock database for reading");
        let messages = repo.select_messages_by_token(token)?;

        return Ok(json!(messages));

    });

    // Sending messages
    app.at("/messages").post(|mut req: Request<State>| async move {

        let token = get_token(req)?;
        let body: model::PostMessageRequest = req.body_json().await?;

        return Ok("Message Added"); // TODO: return new message
    });



    app.listen("127.0.0.1:8080").await?;
    Ok(())
}



fn get_token(req: Request<_>) -> Result<String, tide::Error> {
    let authorization_words = req.header("Authorization")
        .ok_or(tide::Error::from_str(401, "Authorization token is not provided"))?
        .as_str()
        .split_whitespace();

    let authorization_is_basic = authorization_words.next()
        .ok_or(tide::Error::from_str(400, "Unexpected end of Authorization token"))?
        .eq("Basic");

    if !authorization_is_basic { 
        return tide::Error::from_str(400, "Authroization type is not Basic")
    }
    
    return authorization_words.next()
        .ok_or(tide::Error::from_str(400, "Unexpected end of Authorization token"))

}
