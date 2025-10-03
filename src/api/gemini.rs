use base64::{Engine as _, engine::general_purpose::STANDARD};

use poem::web::Data; // Use poem::web::Data for the database connection
use poem_openapi::auth::Bearer;
use poem_openapi::{Object, OpenApi, SecurityScheme, payload::Json, payload::PlainText};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use entity::{helper_app, users};
use crate::api::crypto::decrypt;
use sea_orm::{DatabaseConnection, entity::*, query::*};
use crate::api::memo_api_store_ops::get_user_from_token; 

pub struct GeminiApi;



// --- Structs for API Payloads ---

#[derive(Debug, Deserialize, Object)]
pub struct AudioBufferRequest {
    pub audio_bytes: Vec<u8>,
}

#[derive(Debug, Deserialize, Object)]
pub struct TranslateRequest {
    pub lang: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Object)]
pub struct GenerateTitle {
    pub transcript: String,
}

#[derive(Debug, Deserialize, Object)]
pub struct SummaryRequest {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub exp: usize,
}

// --- Security Scheme Definition for Swagger ---

#[derive(SecurityScheme)]
#[oai(ty = "bearer", bearer_format = "JWT")]
struct ApiKeyAuth(Bearer);

#[OpenApi]
impl GeminiApi {
    #[oai(path = "/transcribe", method = "post")]
    async fn transcribe_audio(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>, // Use poem::web::Data
        Json(payload): Json<AudioBufferRequest>,
    ) -> PlainText<String> {
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user) => user,
            Err(err) => return PlainText(format!("User fetch error: {}", err.0.message)),
        };

        let gemini_api_key = match get_decrypted_gemini_key(&user, db.0).await {
            Ok(key) => key,
            Err(msg) => return PlainText(msg),
        };

        match transcribe_with_gemini(&payload.audio_bytes, &gemini_api_key).await {
            Ok(transcription) => PlainText(transcription),
            Err(err) => PlainText(format!("Transcription Error: {}", err)),
        }
    }

    #[oai(path = "/translate", method = "post")]
    async fn gemini_translate(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>, // <-- FIX: Add DB connection
        Json(payload): Json<TranslateRequest>,
    ) -> PlainText<String> {
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user) => user,
            Err(err) => return PlainText(format!("User fetch error: {}", err.0.message)),
        };

        let gemini_api_key = match get_decrypted_gemini_key(&user, db.0).await {
            Ok(key) => key,
            Err(msg) => return PlainText(msg),
        };

        match translate_with_gemini(&payload.text, &payload.lang, &gemini_api_key).await { // <-- FIX: Pass key
            Ok(result) => PlainText(result),
            Err(err) => PlainText(format!("Error: {}", err)),
        }
    }

    #[oai(path = "/summary", method = "post")]
    async fn gemini_client(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>, // <-- FIX: Add DB connection
        Json(payload): Json<SummaryRequest>,
    ) -> PlainText<String> {
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user) => user,
            Err(err) => return PlainText(format!("User fetch error: {}", err.0.message)),
        };
        
        let gemini_api_key = match get_decrypted_gemini_key(&user, db.0).await {
            Ok(key) => key,
            Err(msg) => return PlainText(msg),
        };

        match summarize_text(&payload.text, &gemini_api_key).await { // <-- FIX: Pass key
            Ok(result) => PlainText(result),
            Err(err) => PlainText(format!("Error: {}", err)),
        }
    }

    #[oai(path = "/generate_memo_name", method = "post")]
    async fn gemini_generate_memo_name(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>, // <-- FIX: Add DB connection
        Json(payload): Json<GenerateTitle>,
    ) -> PlainText<String> {
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user) => user,
            Err(err) => return PlainText(format!("User fetch error: {}", err.0.message)),
        };
        
        let gemini_api_key = match get_decrypted_gemini_key(&user, db.0).await {
            Ok(key) => key,
            Err(msg) => return PlainText(msg),
        };
        
        match generate_title(&payload.transcript, &gemini_api_key).await { // <-- FIX: Pass key
            Ok(result) => PlainText(result),
            Err(err) => PlainText(format!("Error: {}", err)),
        }
    }
}


// --- Refactored Helper Function for fetching the key ---
// This function avoids code duplication in your API handlers.
async fn get_decrypted_gemini_key(user: &users::Model, db: &DatabaseConnection) -> Result<String, String> {
    let key_record = helper_app::Entity::find()
        .filter(helper_app::Column::UserId.eq(user.id))
        .one(db)
        .await
        .map_err(|e| format!("Database error while fetching API key: {}", e))?
        .ok_or_else(|| "Gemini API key not found for this user".to_string())?;

    let decrypted_key = key_record
        .gemini_key
        .as_deref()
        .and_then(|k| decrypt(k).ok())
        .ok_or_else(|| "Failed to decrypt Gemini API key".to_string())?;

    Ok(decrypted_key)
}





// --- Gemini Client and Helper Functions ---
// Ensure these functions correctly receive the api_key parameter.

pub async fn gemini_client(contents: serde_json::Value, key: &str) -> Result<String, String> {
    let client = Client::new();

    let res = client
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent")
        .query(&[("key", key)]) // Key is used here
        .json(&serde_json::json!({ "contents": [contents] }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let error_body = res
            .text()
            .await
            .unwrap_or_else(|_| "Could not read error body".to_string());
        return Err(format!("Gemini API request failed: {}", error_body));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

    if let Some(text) = json
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
    {
        Ok(text.trim().to_string()) // Trim whitespace from response
    } else {
        Err(format!(
            "Failed to parse Gemini API response. Full response: {}",
            json
        ))
    }
}

pub async fn transcribe_with_gemini(audio_bytes: &[u8], api_key: &str) -> Result<String, String> {
    let base64_audio = STANDARD.encode(audio_bytes);
    let content = serde_json::json!({
        "parts": [
            { "text": "Please transcribe this audio." },
            {
                "inline_data": {
                    "mime_type": "audio/wav",
                    "data": base64_audio
                }
            }
        ]
    });
    gemini_client(content, api_key).await
}

pub async fn translate_with_gemini(text: &str, target_lang: &str, api_key: &str) -> Result<String, String> {
    let content = serde_json::json!({
        "parts": [
            { "text": format!( "Translate the following text to {}. Return only the translated text without any extra formatting or explanation:\n\n{}", target_lang, text) }
        ]
    });
    gemini_client(content, api_key).await
}

pub async fn summarize_text(text: &str, api_key: &str) -> Result<String, String> {
    let content = serde_json::json!({
        "parts": [
            { "text": format!("Provide a concise summary of the following text. Keep it brief and capture the main points:\n\n{}", text) }
        ]
    });
    gemini_client(content, api_key).await
}

pub async fn generate_title(transcript: &str, api_key: &str) -> Result<String, String> {
    let content = serde_json::json!({
        "parts":[
            {"text":format!("Generate a short, descriptive title (2-4 words) for this voice memo based on its content. Return only the title:\n\n{}",transcript)}
        ]
    });
    gemini_client(content, api_key).await
}