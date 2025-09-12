
use chrono::Utc;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use poem::{web::Data, Result, error::{BadRequest, NotFound, Unauthorized}};
use poem_openapi::{payload::Json, param::Path, Object, OpenApi, SecurityScheme};
use poem_openapi::auth::Bearer;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json; // Added for robust JSON handling of tags
use uuid::Uuid;
use std::error::Error as StdError;
use std::fmt;

use entity::{users, voice_memos1};

// --- Custom Error for Poem ---
#[derive(Debug)]
struct ApiError(String);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl StdError for ApiError {}


// --- Constants ---
const JWT_SECRET: &str = "point";

// --- API Structs ---

// Memo Payloads and Responses
#[derive(Object, Debug, Deserialize)]
pub struct MemoInput {
    pub id: Option<String>,
    pub title: String,
    pub transcript: Option<String>,
    pub translate: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>, // Correctly defined as a vector of strings
    pub duration: String,
    pub audio_blob: Option<Vec<u8>>,
}

#[derive(Object, Debug, Deserialize)]
pub struct SaveAudioMemoPayload {
    pub title: String,
    pub duration: String,
    pub audio_blob: String,
}

#[derive(Object, Debug, Deserialize)]
pub struct MemoUpdate {
    pub title: Option<String>,
    pub transcript: Option<String>,
    pub translate: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>, // Correctly defined as a vector of strings
}

#[derive(Object, Serialize)]
pub struct MemoResponse {
    pub message: String,
    pub memo_id: String,
}

#[derive(Object, Serialize)]
pub struct MemoOutput {
    pub id: String,
    pub title: String,
    pub transcript: Option<String>,
    pub translate: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>, 
    pub duration: String,
    pub created_at: String,
    pub audio_blob: Option<Vec<u8>>,
}

// --- JWT Claims ---
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub exp: usize,
}

// --- Security Scheme Definition for Swagger ---
#[derive(SecurityScheme)]
#[oai(
    ty = "bearer",
    bearer_format = "JWT",
    
)]
struct ApiKeyAuth(Bearer);

// --- API Definition ---
pub struct MemoApi;

#[OpenApi]
impl MemoApi {
    #[oai(path = "/save_memo", method = "post")]
    async fn save_memo(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
        Json(payload): Json<MemoInput>,
    ) -> Json<MemoResponse> {
        let claims = match validate_token(&auth.0.token) {
            Ok(c) => c,
            Err(msg) => return Json(MemoResponse { message: msg, memo_id: "".to_string() }),
        };

        let user_id = match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(_) => return Json(MemoResponse { message: "Invalid user ID format".to_string(), memo_id: "".to_string() }),
        };

        if let Ok(None) = users::Entity::find_by_id(user_id).one(db.0).await {
             return Json(MemoResponse { message: format!("User {} not found", user_id), memo_id: "".to_string() });
        }

        if payload.title.trim().is_empty() || payload.duration.trim().is_empty() {
            return Json(MemoResponse { message: "Title and duration are required".to_string(), memo_id: "".to_string() });
        }

        let audio_blob_bytes = payload.audio_blob;
        
        
        // Serialize tags vector into a JSON string for database storage
        let tags_json_string = payload.tags.as_ref().and_then(|v| serde_json::to_string(v).ok());

        // Helper to ensure empty strings for optional fields become NULL in the DB
        let clean_field = |val: Option<String>| val.and_then(|s| if s.trim().is_empty() { None } else { Some(s) });

        // UPDATE FLOW
        if let Some(ref id_str) = payload.id {
            if let Ok(memo_uuid) = Uuid::parse_str(id_str) {
                if let Ok(Some(existing)) = voice_memos1::Entity::find_by_id(memo_uuid).one(db.0).await {
                    if existing.user_id != user_id {
                        return Json(MemoResponse { message: "Unauthorized".to_string(), memo_id: "".to_string() });
                    }

                    let mut update_model: voice_memos1::ActiveModel = existing.into();
                    update_model.title = Set(payload.title);
                    update_model.transcript = Set(clean_field(payload.transcript));
                    update_model.translate = Set(clean_field(payload.translate));
                    update_model.summary = Set(clean_field(payload.summary));
                    update_model.tags = Set(tags_json_string); // Store tags as JSON string
                    update_model.duration = Set(payload.duration);
                    if let Some(blob) = audio_blob_bytes {
                        update_model.audio_blob = Set(Some(blob));
                    }

                    return match update_model.update(db.0).await {
                        Ok(updated) => Json(MemoResponse { message: "Memo updated".to_string(), memo_id: updated.id.to_string() }),
                        Err(e) => Json(MemoResponse { message: format!("Update failed: {}", e), memo_id: "".to_string() }),
                    };
                }
            }
        }

        // INSERT FLOW
        let new_memo = voice_memos1::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            title: Set(payload.title),
            audio_blob: Set(audio_blob_bytes),
            transcript: Set(clean_field(payload.transcript)),
            translate: Set(clean_field(payload.translate)),
            summary: Set(clean_field(payload.summary)),
            tags: Set(tags_json_string), // Store tags as JSON string
            duration: Set(payload.duration),
            created_at: Set(Utc::now().naive_utc()),
        };

        match new_memo.insert(db.0).await {
            Ok(saved) => Json(MemoResponse { message: "Memo saved".to_string(), memo_id: saved.id.to_string() }),
            Err(e) => Json(MemoResponse { message: format!("Save failed: {}", e), memo_id: "".to_string() }),
        }
    }

    #[oai(path = "/get_memos", method = "get")]
    async fn get_memos(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
    ) -> Json<Vec<MemoOutput>> {
        let claims = match validate_token(&auth.0.token) {
            Ok(c) => c,
            Err(_) => return Json(vec![]),
        };

        let user_id = match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(_) => return Json(vec![]),
        };

        let memos = match voice_memos1::Entity::find()
            .filter(voice_memos1::Column::UserId.eq(user_id))
            .all(db.0)
            .await {
            Ok(memos) => memos,
            Err(_) => return Json(vec![]),
        };

        let response = memos.into_iter().map(|memo| MemoOutput {
            id: memo.id.to_string(),
            title: memo.title,
            transcript: memo.transcript,
            translate: memo.translate,
            summary: memo.summary,
            // Deserialize tags from JSON string back to a vector
            tags: memo.tags.and_then(|json_str| serde_json::from_str(&json_str).ok()),
            duration: memo.duration,
            created_at: memo.created_at.to_string(),
            audio_blob: memo.audio_blob,
        }).collect();

        Json(response)
    }
    
    #[oai(path = "/get_memo/:memo_id", method = "get")]
    async fn get_memo_by_id(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
        Path(memo_id): Path<String>,
    ) -> Result<Json<MemoOutput>> {
        let claims = match validate_token(&auth.0.token) {
            Ok(claims) => claims,
            Err(e) => return Err(Unauthorized(ApiError(e))),
        };
        let user_id = Uuid::parse_str(&claims.sub).map_err(BadRequest)?;
        let memo_uuid = Uuid::parse_str(&memo_id).map_err(BadRequest)?;

        let memo = voice_memos1::Entity::find_by_id(memo_uuid)
            .filter(voice_memos1::Column::UserId.eq(user_id))
            .one(db.0)
            .await
            .map_err(poem::error::InternalServerError)?
            .ok_or_else(|| NotFound(ApiError("Memo not found or access denied".to_string())))?;

        let response = MemoOutput {
            id: memo.id.to_string(),
            title: memo.title,
            transcript: memo.transcript,
            translate: memo.translate,
            summary: memo.summary,
            // Deserialize tags from JSON string back to a vector
            tags: memo.tags.and_then(|json_str| serde_json::from_str(&json_str).ok()),
            duration: memo.duration,
            created_at: memo.created_at.to_string(),
            audio_blob: memo.audio_blob,
        };

        Ok(Json(response))
    }

    #[oai(path = "/update_memo/:memo_id", method = "patch")]
    async fn update_memo(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
        Path(memo_id): Path<String>,
        Json(payload): Json<MemoUpdate>,
    ) -> Json<MemoResponse> {
        let claims = match validate_token(&auth.0.token) {
            Ok(c) => c,
            Err(msg) => return Json(MemoResponse { message: msg, memo_id: "".to_string() }),
        };

        let user_id = match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(_) => return Json(MemoResponse { message: "Invalid user ID".to_string(), memo_id: "".to_string() }),
        };

        let memo_uuid = match Uuid::parse_str(&memo_id) {
            Ok(id) => id,
            Err(_) => return Json(MemoResponse { message: "Invalid memo ID".to_string(), memo_id: "".to_string() }),
        };

        let memo = match voice_memos1::Entity::find_by_id(memo_uuid)
            .filter(voice_memos1::Column::UserId.eq(user_id))
            .one(db.0)
            .await {
            Ok(Some(memo)) => memo,
            Ok(None) => return Json(MemoResponse { message: "Memo not found or access denied".to_string(), memo_id: "".to_string() }),
            Err(e) => return Json(MemoResponse { message: format!("DB Error: {}", e), memo_id: "".to_string() }),
        };

        let mut active_memo: voice_memos1::ActiveModel = memo.into();
        let clean_field = |val: String| if val.trim().is_empty() { None } else { Some(val) };

        if let Some(title) = payload.title {
            if !title.trim().is_empty() {
                active_memo.title = Set(title);
            }
        }
        
        if let Some(transcript) = payload.transcript {
            active_memo.transcript = Set(clean_field(transcript));
        }
        if let Some(translate) = payload.translate {
            active_memo.translate = Set(clean_field(translate));
        }
        if let Some(summary) = payload.summary {
            active_memo.summary = Set(clean_field(summary));
        }
        if let Some(tags_vec) = payload.tags {
            // Serialize the vector to a JSON string before saving.
            active_memo.tags = Set(serde_json::to_string(&tags_vec).ok());
        }

        match active_memo.update(db.0).await {
            Ok(updated) => Json(MemoResponse { message: "Memo updated successfully".to_string(), memo_id: updated.id.to_string() }),
            Err(e) => Json(MemoResponse { message: format!("Failed to update memo: {}", e), memo_id: "".to_string() }),
        }
    }

    #[oai(path = "/delete_memo/:memo_id", method = "delete")]
    async fn delete_memo(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
        Path(memo_id): Path<String>,
    ) -> Json<MemoResponse> {
        let claims = match validate_token(&auth.0.token) {
            Ok(c) => c,
            Err(msg) => return Json(MemoResponse { message: msg, memo_id: "".to_string() }),
        };

        let user_id = match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(_) => return Json(MemoResponse { message: "Invalid user ID".to_string(), memo_id: "".to_string() }),
        };
        
        let memo_uuid = match Uuid::parse_str(&memo_id) {
            Ok(id) => id,
            Err(_) => return Json(MemoResponse { message: "Invalid memo ID".to_string(), memo_id: "".to_string() }),
        };

        let result = voice_memos1::Entity::delete_many()
            .filter(voice_memos1::Column::Id.eq(memo_uuid))
            .filter(voice_memos1::Column::UserId.eq(user_id))
            .exec(db.0)
            .await;

        match result {
            Ok(res) if res.rows_affected > 0 => Json(MemoResponse { message: "Memo deleted".to_string(), memo_id: memo_id }),
            Ok(_) => Json(MemoResponse { message: "Memo not found or access denied".to_string(), memo_id: "".to_string() }),
            Err(e) => Json(MemoResponse { message: format!("Deletion failed: {}", e), memo_id: "".to_string() }),
        }
    }

    #[oai(path = "/delete_all_memos", method = "delete")]
    async fn delete_all_memos(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
    ) -> Json<MemoResponse> {
        let claims = match validate_token(&auth.0.token) {
            Ok(c) => c,
            Err(msg) => return Json(MemoResponse { message: msg, memo_id: "".to_string() }),
        };

        let user_id = match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(_) => return Json(MemoResponse { message: "Invalid user ID".to_string(), memo_id: "".to_string() }),
        };

        match voice_memos1::Entity::delete_many()
            .filter(voice_memos1::Column::UserId.eq(user_id))
            .exec(db.0)
            .await
        {
            Ok(delete_result) => Json(MemoResponse {
                message: format!("Deleted {} memo(s)", delete_result.rows_affected),
                memo_id: "".to_string(),
            }),
            Err(e) => Json(MemoResponse {
                message: format!("Failed to delete memos: {}", e),
                memo_id: "".to_string(),
            }),
        }
    }
}

// --- Helper Functions ---

fn validate_token(token: &str) -> Result<Claims, String> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_ref()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|data| data.claims)
    .map_err(|_| "Invalid or expired token".to_string())
}
