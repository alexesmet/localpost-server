use serde::{Deserialize, Serialize};


#[derive(Deserialize)]
pub struct PostMessageRequest {
    pub recipients: Vec<u32>,
    pub text: String
}

#[derive(Serialize, Clone)]
pub struct UserResponse {
    pub id: u32,
    pub name: String,
}

#[derive(Serialize, Clone)]
pub struct MessageResponse {
    pub id: u32,
    pub text: String,
    pub timestamp: i64,
    pub sender_name: String,
    pub sender_id: u32,
}


#[derive(Debug)]
pub struct UserCredentials {
    pub username: String,
    pub password: String
}
