use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng}, 
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose}; 
const ENCRYPTION_KEY: &[u8; 32] = b"01234567890123456789012345678901"; 

pub fn encrypt(plain_text: &str) -> Result<String, String> {
   
    let cipher = Aes256Gcm::new(ENCRYPTION_KEY.into());

  
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); 
    
   
    let ciphertext = cipher
        .encrypt(&nonce, plain_text.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

   
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    
   
    Ok(general_purpose::STANDARD.encode(combined))
}

pub fn decrypt(encoded: &str) -> Result<String, String> {
    
    let combined = general_purpose::STANDARD.decode(encoded)
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    
  
    if combined.len() < 12 {
        return Err("Invalid encrypted payload: too short".to_string());
    }

    
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    
    let cipher = Aes256Gcm::new(ENCRYPTION_KEY.into());

   
    let decrypted_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))?;

    
    String::from_utf8(decrypted_bytes)
        .map_err(|e| format!("UTF-8 decode error: {}", e))
}