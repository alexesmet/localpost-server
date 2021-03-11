use serde::{Deserialize, Serialize};


#[derive(Deserialize)]
pub struct PostMessageRequest {
    pub text: String
}

#[derive(Serialize, Clone, Debug)]
pub struct UserResponse {
    pub id: u32,
    pub name: String,
    pub username: String,
    pub password: String,
    pub admin: bool
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
