use crate::model::{AuthResponse, Claims, LoginRequest};
use axum::{
    extract::{Json, Request},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{Json as ResponseJson, Response},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};

// In production, use a secure secret from environment variables
const JWT_SECRET: &str = "your-secret-key-change-this-in-production";

pub fn create_token(username: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: username.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_ref()),
    )
}

pub fn validate_token(token: &str) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_ref()),
        &Validation::default(),
    )
}

pub async fn login(
    Json(payload): Json<LoginRequest>,
) -> Result<ResponseJson<AuthResponse>, StatusCode> {
    if payload.username != "admin" {
        return Err(StatusCode::UNAUTHORIZED);
    }

    match create_token(&payload.username) {
        Ok(token) => Ok(ResponseJson(AuthResponse { token })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract the Authorization header
    let auth_header = headers
        .get("authorization")
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header {
        // Check if it starts with "Bearer "
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            // Validate the token
            match validate_token(token) {
                Ok(_) => {
                    // Token is valid, proceed with the request
                    Ok(next.run(request).await)
                }
                Err(_) => Err(StatusCode::UNAUTHORIZED),
            }
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
