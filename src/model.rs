use serde::{Deserialize, Serialize};


#[derive(Deserialize)]
pub struct PostMessageRequest {
    pub recipients: Vec<u32>,
    pub text: String
}

#[derive(Serialize)]
pub struct EmbeddedRecipient {
    pub id: u32,
    pub name: String,
    pub color: String
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub id: u32,
    pub text: String,
    pub timestamp: u32,
    pub sender_name: String,
    pub sender_id: u32,
    pub sender_color: String,
    pub recipients: Vec<EmbeddedRecipient>
}


#[derive(Debug)]
pub struct UserCredentials {
    pub username: String,
    pub password: String
}
