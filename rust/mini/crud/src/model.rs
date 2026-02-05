use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Monster {
    #[serde(skip_deserializing)]
    pub id: u32,

    pub name: String,
    pub age: u32,
    pub alive: bool,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // username
    pub exp: usize,  // expiration time
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
}
