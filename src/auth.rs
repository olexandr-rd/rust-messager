use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const SECRET: &[u8] = b"your_secret_key"; // Change to your actual secret key

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,         // Username
    exp: usize,          // Expiration time
}

// Generate JWT token
pub fn generate_jwt(username: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as usize
        + 3600; // Token expires in 1 hour

    let claims = Claims {
        sub: username.to_owned(),
        exp: expiration,
    };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(SECRET))
}

// Validate JWT token
pub fn validate_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(SECRET),
        &Validation::default(),
    )
        .map(|data| data.claims)
}
