use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use poem::web::Data;
use poem_openapi::{Object, OpenApi, SecurityScheme, auth::Bearer, payload::Json, ApiResponse};
use sea_orm::{DatabaseConnection, Set, entity::*, query::*, ActiveModelTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::api::crypto::{encrypt, decrypt};

use entity::{helper_app, users};


const JWT_SECRET: &str = "point";


#[derive(Debug, Deserialize, Serialize, Object)]
pub struct ApiKeyPayload {
    pub gemini_api_key: Option<String>,
    pub elevenlabs_api_key: Option<String>,
}

#[derive(Debug, Serialize, Object)]
pub struct ApiKeyResponse {
    pub gemini_api_key: Option<String>,
    pub elevenlabs_api_key: Option<String>,
    pub message: String,
}

// NEW: Structs for helper status
#[derive(Debug, Deserialize, Object)]
pub struct HelperStatusPayload {
    pub status: bool,
}

#[derive(Debug, Serialize, Object)]
pub struct HelperStatusResponse {
    pub status: bool,
    pub message: String,
}

// NEW: Struct for delete responses
#[derive(Debug, Serialize, Object)]
pub struct DeleteResponse {
    pub message: String,
}


#[derive(ApiResponse)]
enum GetApiResponse {
    #[oai(status = 200)]
    Ok(Json<ApiKeyResponse>),
    #[oai(status = 404)]
    NotFound(Json<ApiKeyResponse>),
    #[oai(status = 401)]
    Unauthorized(Json<ApiKeyResponse>),
    #[oai(status = 500)]
    InternalServerError(Json<ApiKeyResponse>),
}

#[derive(ApiResponse)]
enum SaveApiResponse {
    #[oai(status = 200)]
    Ok(Json<ApiKeyResponse>),
    #[oai(status = 401)]
    Unauthorized(Json<ApiKeyResponse>),
    #[oai(status = 500)]
    InternalServerError(Json<ApiKeyResponse>),
}

// NEW: API Responses for helper status
#[derive(ApiResponse)]
enum HelperStatusUpdateResponse {
    #[oai(status = 200)]
    Ok(Json<HelperStatusResponse>),
    #[oai(status = 401)]
    Unauthorized,
    #[oai(status = 500)]
    InternalServerError(Json<String>),
}

#[derive(ApiResponse)]
enum HelperStatusGetResponse {
    #[oai(status = 200)]
    Ok(Json<HelperStatusResponse>),
    #[oai(status = 401)]
    Unauthorized,
    #[oai(status = 500)]
    InternalServerError(Json<String>),
}

// NEW: API Response for delete operations
#[derive(ApiResponse)]
enum DeleteApiResponse {
    #[oai(status = 200)]
    Ok(Json<DeleteResponse>),
    #[oai(status = 401)]
    Unauthorized,
    #[oai(status = 404)]
    NotFound(Json<String>),
    #[oai(status = 500)]
    InternalServerError(Json<String>),
}


#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub exp: usize,
}


#[derive(SecurityScheme)]
#[oai(ty = "bearer", bearer_format = "JWT")]
struct ApiKeyAuth(Bearer);


pub async fn get_user_from_token(
    token: &str,
    db: &DatabaseConnection
) -> Result<users::Model, Json<ApiKeyResponse>> {
    // 1. Validate the JWT token
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_ref()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|data| data.claims)
    .map_err(|_| Json(ApiKeyResponse {
        gemini_api_key: None,
        elevenlabs_api_key: None,
        message: "Invalid or expired token".to_string(),
    }))?;

    // 2. Parse the user ID from the token's subject
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| Json(ApiKeyResponse {
        gemini_api_key: None,
        elevenlabs_api_key: None,
        message: "Invalid user ID format in token".to_string(),
    }))?;

    // 3. Find the user in the database
    users::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|e| {
            tracing::error!("Database error while fetching user: {:?}", e);
            Json(ApiKeyResponse {
                gemini_api_key: None,
                elevenlabs_api_key: None,
                message: "Failed to verify user due to a database error".to_string(),
            })
        })?
        .ok_or_else(|| Json(ApiKeyResponse {
            gemini_api_key: None,
            elevenlabs_api_key: None,
            message: format!("User {} not found in database", user_id),
        }))
}


pub struct Api;

#[OpenApi]
impl Api {
    
    #[oai(path = "/api_keys/save", method = "post")]
    async fn save_api_keys(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
        Json(payload): Json<ApiKeyPayload>,
    ) -> SaveApiResponse {
      
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user_model) => user_model,
            Err(error_response) => return SaveApiResponse::Unauthorized(error_response),
        };

        
        let encrypted_gemini = match payload.gemini_api_key.as_deref().map(encrypt).transpose() {
            Ok(key) => key,
            Err(e) => {
                tracing::error!("Failed to encrypt Gemini key: {}", e);
                return SaveApiResponse::InternalServerError(Json(ApiKeyResponse {
                    gemini_api_key: None, elevenlabs_api_key: None,
                    message: "Failed to process Gemini API key.".to_string(),
                }));
            }
        };
        let encrypted_elevenlabs = match payload.elevenlabs_api_key.as_deref().map(encrypt).transpose() {
            Ok(key) => key,
            Err(e) => {
                tracing::error!("Failed to encrypt ElevenLabs key: {}", e);
                return SaveApiResponse::InternalServerError(Json(ApiKeyResponse {
                    gemini_api_key: None, elevenlabs_api_key: None,
                    message: "Failed to process ElevenLabs API key.".to_string(),
                }));
            }
        };

        
        let existing_record = match helper_app::Entity::find()
            .filter(helper_app::Column::UserId.eq(user.id))
            .one(db.0)
            .await
        {
            Ok(record) => record,
            Err(e) => {
                tracing::error!("DB error finding API keys: {:?}", e);
                return SaveApiResponse::InternalServerError(Json(ApiKeyResponse {
                    gemini_api_key: None, elevenlabs_api_key: None,
                    message: "Failed to check for existing API keys.".to_string(),
                }));
            }
        };

        
        let result = match existing_record {
            Some(model) => {
                
                let mut active_model: helper_app::ActiveModel = model.into();
                if let Some(key) = encrypted_gemini {
                    active_model.gemini_key = Set(Some(key));
                }
                if let Some(key) = encrypted_elevenlabs {
                    active_model.elevenlabs_key = Set(Some(key));
                }
                active_model.timestamp = Set(Utc::now().naive_utc());
                active_model.update(db.0).await
            }
            None => {
                
                let new_model = helper_app::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    user_id: Set(user.id),
                    gemini_key: Set(encrypted_gemini),
                    elevenlabs_key: Set(encrypted_elevenlabs),
                    action: Set("api_keys_save".to_string()),
                    timestamp: Set(Utc::now().naive_utc()),
                    helper_status: Set(false), // Default status
                };
                new_model.insert(db.0).await
            }
        };

        match result {
            Ok(_) => SaveApiResponse::Ok(Json(ApiKeyResponse {
                
                gemini_api_key: payload.gemini_api_key,
                elevenlabs_api_key: payload.elevenlabs_api_key,
                message: "API keys saved successfully".to_string(),
            })),
            Err(e) => {
                tracing::error!("Failed to save API keys to DB: {:?}", e);
                SaveApiResponse::InternalServerError(Json(ApiKeyResponse {
                    gemini_api_key: None, elevenlabs_api_key: None,
                    message: "Failed to save API keys.".to_string(),
                }))
            }
        }
    }

    
    #[oai(path = "/api_keys/get", method = "get")]
    async fn get_api_keys(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
    ) -> GetApiResponse {
      
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user_model) => user_model,
            Err(error_response) => return GetApiResponse::Unauthorized(error_response),
        };

        let keys_record = match helper_app::Entity::find()
            .filter(helper_app::Column::UserId.eq(user.id))
            .one(db.0)
            .await
        {
            Ok(Some(record)) => record,
            Ok(None) => return GetApiResponse::NotFound(Json(ApiKeyResponse {
                gemini_api_key: None, elevenlabs_api_key: None,
                message: "No API keys found for this user.".to_string(),
            })),
            Err(e) => {
                tracing::error!("Failed to fetch API keys: {:?}", e);
                return GetApiResponse::InternalServerError(Json(ApiKeyResponse {
                    gemini_api_key: None, elevenlabs_api_key: None,
                    message: "Failed to fetch API keys.".to_string(),
                }));
            }
        };

        
        let gemini_key = keys_record.gemini_key.as_deref().and_then(|k| decrypt(k).ok());
        let elevenlabs_key = keys_record.elevenlabs_key.as_deref().and_then(|k| decrypt(k).ok());

        GetApiResponse::Ok(Json(ApiKeyResponse {
            gemini_api_key: gemini_key,
            elevenlabs_api_key: elevenlabs_key,
            message: "API keys retrieved successfully".to_string(),
        }))
    }

    // NEW: Endpoint to delete Gemini key
    #[oai(path = "/api_keys/gemini", method = "delete")]
    async fn delete_gemini_key(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
    ) -> DeleteApiResponse {
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user_model) => user_model,
            Err(_) => return DeleteApiResponse::Unauthorized,
        };

        let existing_record = match helper_app::Entity::find()
            .filter(helper_app::Column::UserId.eq(user.id))
            .one(db.0)
            .await
        {
            Ok(Some(record)) => record,
            Ok(None) => return DeleteApiResponse::NotFound(Json("API key record not found for user.".to_string())),
            Err(e) => return DeleteApiResponse::InternalServerError(Json(e.to_string())),
        };

        let mut active_model: helper_app::ActiveModel = existing_record.into();
        active_model.gemini_key = Set(None);
        
        match active_model.update(db.0).await {
            Ok(_) => DeleteApiResponse::Ok(Json(DeleteResponse {
                message: "Gemini API key deleted successfully".to_string(),
            })),
            Err(e) => DeleteApiResponse::InternalServerError(Json(e.to_string())),
        }
    }

    // NEW: Endpoint to delete ElevenLabs key
    #[oai(path = "/api_keys/elevenlabs", method = "delete")]
    async fn delete_elevenlabs_key(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
    ) -> DeleteApiResponse {
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user_model) => user_model,
            Err(_) => return DeleteApiResponse::Unauthorized,
        };

        let existing_record = match helper_app::Entity::find()
            .filter(helper_app::Column::UserId.eq(user.id))
            .one(db.0)
            .await
        {
            Ok(Some(record)) => record,
            Ok(None) => return DeleteApiResponse::NotFound(Json("API key record not found for user.".to_string())),
            Err(e) => return DeleteApiResponse::InternalServerError(Json(e.to_string())),
        };

        let mut active_model: helper_app::ActiveModel = existing_record.into();
        active_model.elevenlabs_key = Set(None);
        
        match active_model.update(db.0).await {
            Ok(_) => DeleteApiResponse::Ok(Json(DeleteResponse {
                message: "ElevenLabs API key deleted successfully".to_string(),
            })),
            Err(e) => DeleteApiResponse::InternalServerError(Json(e.to_string())),
        }
    }

    #[oai(path = "/helper/status", method = "post")]
    async fn update_helper_status(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
        Json(payload): Json<HelperStatusPayload>,
    ) -> HelperStatusUpdateResponse {
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user_model) => user_model,
            Err(_) => return HelperStatusUpdateResponse::Unauthorized,
        };

        let existing_record = match helper_app::Entity::find()
            .filter(helper_app::Column::UserId.eq(user.id))
            .one(db.0)
            .await
        {
            Ok(record) => record,
            Err(e) => {
                tracing::error!("DB error finding helper_app record: {:?}", e);
                return HelperStatusUpdateResponse::InternalServerError(Json(e.to_string()));
            }
        };

        let result = match existing_record {
            Some(model) => {
                let mut active_model: helper_app::ActiveModel = model.into();
                active_model.helper_status = Set(payload.status);
                active_model.update(db.0).await
            }
            None => {
                let new_model = helper_app::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    user_id: Set(user.id),
                    helper_status: Set(payload.status),
                    action: Set("helper_status_update".to_string()),
                    timestamp: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };
                new_model.insert(db.0).await
            }
        };

        match result {
            Ok(_) => HelperStatusUpdateResponse::Ok(Json(HelperStatusResponse {
                status: payload.status,
                message: "Helper status updated successfully".to_string(),
            })),
            Err(e) => {
                tracing::error!("Failed to update helper status: {:?}", e);
                HelperStatusUpdateResponse::InternalServerError(Json(e.to_string()))
            }
        }
    }

    #[oai(path = "/helper/status", method = "get")]
    async fn get_helper_status(
        &self,
        auth: ApiKeyAuth,
        db: Data<&DatabaseConnection>,
    ) -> HelperStatusGetResponse {
        let user = match get_user_from_token(&auth.0.token, db.0).await {
            Ok(user_model) => user_model,
            Err(_) => return HelperStatusGetResponse::Unauthorized,
        };

        match helper_app::Entity::find()
            .filter(helper_app::Column::UserId.eq(user.id))
            .one(db.0)
            .await
        {
            Ok(Some(record)) => {
                HelperStatusGetResponse::Ok(Json(HelperStatusResponse {
                    status: record.helper_status,
                    message: "Helper status retrieved successfully".to_string(),
                }))
            }
            Ok(None) => {
                // If no record exists, the status is effectively false
                HelperStatusGetResponse::Ok(Json(HelperStatusResponse {
                    status: false,
                    message: "No helper record found; returning default status.".to_string(),
                }))
            }
            Err(e) => {
                tracing::error!("Failed to fetch helper status: {:?}", e);
                HelperStatusGetResponse::InternalServerError(Json(e.to_string()))
            }
        }
    }
}
