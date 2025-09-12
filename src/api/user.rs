// In your Cargo.toml, you need to add the validator crate:
// validator = { version = "0.16", features = ["derive"] }

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header };
use poem::{
    error::{BadRequest, Conflict, Unauthorized},
    web::Data,
    Result,
};
use poem_openapi::{ payload::Json, Object, OpenApi};
use sea_orm::{entity::*, query::*, DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate; // Import the validation trait

use entity::users::{self, Entity as Users};
use std::error::Error as StdError;
use std::fmt;
use bcrypt::{hash, DEFAULT_COST, verify};


// --- Custom Error for Poem ---
#[derive(Debug)]
struct ApiError(String);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl StdError for ApiError {}


// --- API Structs ---

#[derive(Object, Deserialize, Validate)] // Derive Validate for the payload
pub struct SignupPayload {
    #[validate(length(min = 3, message = "Username must be at least 3 characters long"))]
    username: String,
    #[validate(email(message = "Please provide a valid email address"))]
    email: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters long"))]
    password: String,
}

#[derive(Object, Serialize)]
pub struct SignupResponse {
    message: String,
    user_id: String,
}

#[derive(Object, Deserialize)]
pub struct LoginPayload {
    email: String,
    password: String,
}

#[derive(Object, Serialize)]
pub struct LoginResponse {
    message: String,
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    username:String,
    email: String,
    exp: usize,
}

pub struct UserApi;

#[OpenApi]
impl UserApi {
    /// Signup a new user
    #[oai(path = "/signup", method = "post")]
    async fn signup(
        &self,
        db: Data<&DatabaseConnection>,
        Json(payload): Json<SignupPayload>,
    ) -> Result<Json<SignupResponse>> {
        // 1. Validate the incoming payload based on the rules in the struct
        payload.validate().map_err(BadRequest)?;

        // 2. Check if a user with this email already exists
        let existing_user = Users::find()
            .filter(users::Column::Email.eq(payload.email.clone()))
            .one(db.0)
            .await
            .map_err(poem::error::InternalServerError)?;

        if existing_user.is_some() {
            // If a user is found, return a 409 Conflict error
            return Err(Conflict(ApiError(
                "User with this email already exists".to_string(),
            )));
        }

        // 3. Hash the password before saving
        let hashed_password = hash(&payload.password, DEFAULT_COST)
            .map_err(|_| poem::error::InternalServerError(ApiError("Failed to hash password".to_string())))?;

        // 4. Create the new user if validation passes and the user doesn't exist
        let user = users::ActiveModel {
            id: Set(Uuid::new_v4()),
            username: Set(payload.username),
            email: Set(payload.email),
            password: Set(hashed_password),
            created_at: Set(chrono::Utc::now().naive_utc()),
        };

        let saved = user.insert(db.0).await.map_err(|e| {
            poem::error::InternalServerError(ApiError(format!("Failed to create user: {}", e)))
        })?;

        Ok(Json(SignupResponse {
            message: "User created successfully".to_string(),
            user_id: saved.id.to_string(),
        }))
    }

    #[oai(path = "/login", method = "post")]
    async fn login(
        &self,
        db: Data<&DatabaseConnection>,
        Json(payload): Json<LoginPayload>,
    ) -> Result<Json<LoginResponse>> {
        // Find the user by email
        let user = Users::find()
            .filter(users::Column::Email.eq(payload.email.clone()))
            .one(db.0)
            .await
            .map_err(poem::error::InternalServerError)?
            .ok_or_else(|| Unauthorized(ApiError("Invalid email or password".to_string())))?;

        // Verify the password hash
        let is_valid = verify(&payload.password, &user.password).unwrap_or(false);

        if is_valid {
            // If the password is valid, create a JWT token
            let expiration = Utc::now()
                .checked_add_signed(Duration::hours(24))
                .expect("Failed to calculate token expiration")
                .timestamp();

            let claims = Claims {
                sub: user.id.to_string(),
                username:user.username.clone(),
                email: user.email.clone(),
                exp: expiration as usize,
            };

            let token = encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret("point".as_ref()),
            )
            .map_err(|_| poem::error::InternalServerError(ApiError("Failed to create token".to_string())))?;

            Ok(Json(LoginResponse {
                message: "Login Successful".to_string(),
                token,
            }))
        } else {
            // If the password is not valid, return an Unauthorized error
            Err(Unauthorized(ApiError(
                "Invalid email or password".to_string(),
            )))
        }
    }
}
