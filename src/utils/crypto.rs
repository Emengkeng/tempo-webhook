use hmac::{Hmac, Mac};
use sha2::Sha256;
use rand::Rng;

type HmacSha256 = Hmac<Sha256>;

/// Generate HMAC signature for webhook payload
pub fn generate_webhook_signature(payload: &str, secret: &str, timestamp: i64) -> String {
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(signed_payload.as_bytes());
    
    let result = mac.finalize();
    let signature = hex::encode(result.into_bytes());
    
    format!("t={},v1={}", timestamp, signature)
}

/// Verify webhook signature (for incoming webhooks from Polar)
pub fn verify_webhook_signature(
    payload: &str,
    signature: &str,
    secret: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Parse signature
    let parts: Vec<&str> = signature.split(',').collect();
    if parts.len() != 2 {
        return Ok(false);
    }
    
    let timestamp = parts[0]
        .strip_prefix("t=")
        .ok_or("Invalid signature format")?
        .parse::<i64>()?;
    let received_sig = parts[1]
        .strip_prefix("v1=")
        .ok_or("Invalid signature format")?;
    
    // Check timestamp (prevent replay attacks - 5 minute window)
    let now = chrono::Utc::now().timestamp();
    if (now - timestamp).abs() > 300 {
        return Ok(false);
    }
    
    // Generate expected signature
    let expected = generate_webhook_signature(payload, secret, timestamp);
    let expected_sig = expected
        .split(',')
        .nth(1)
        .and_then(|s| s.strip_prefix("v1="))
        .ok_or("Failed to parse expected signature")?;
    
    // Constant-time comparison
    Ok(constant_time_eq(received_sig, expected_sig))
}

/// Constant-time string comparison
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    a.as_bytes()
        .iter()
        .zip(b.as_bytes().iter())
        .fold(0, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Generate a random API key
/// Returns: (full_key, key_hash, key_prefix)
pub fn generate_api_key(prefix: &str) -> (String, String, String) {
    let random_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen()).collect();
    let key = format!("{}_{}", prefix, hex::encode(random_bytes));
    let hash = hash_api_key(&key);
    (key, hash, prefix.to_string())
}

/// Hash an API key for storage
pub fn hash_api_key(key: &str) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate a webhook secret
pub fn generate_webhook_secret() -> String {
    let random_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen()).collect();
    format!("whsec_{}", hex::encode(random_bytes))
}

/// Encrypt data using AES-256-GCM (for sensitive data)
pub fn encrypt_data(data: &str, key: &str) -> Result<String, Box<dyn std::error::Error>> {
    // For production, use proper AES-GCM encryption
    // This is a placeholder - you should use the `aes-gcm` crate
    Ok(hex::encode(data.as_bytes()))
}

/// Decrypt data using AES-256-GCM
pub fn decrypt_data(encrypted: &str, key: &str) -> Result<String, Box<dyn std::error::Error>> {
    // For production, use proper AES-GCM decryption
    // This is a placeholder - you should use the `aes-gcm` crate
    let bytes = hex::decode(encrypted)?;
    Ok(String::from_utf8(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_signature() {
        let payload = r#"{"test":"data"}"#;
        let secret = "test_secret";
        let timestamp = 1234567890;
        
        let signature = generate_webhook_signature(payload, secret, timestamp);
        assert!(signature.starts_with("t=1234567890,v1="));
    }

    #[test]
    fn test_verify_webhook_signature() {
        let payload = r#"{"test":"data"}"#;
        let secret = "test_secret";
        let timestamp = chrono::Utc::now().timestamp();
        
        let signature = generate_webhook_signature(payload, secret, timestamp);
        let valid = verify_webhook_signature(payload, &signature, secret).unwrap();
        
        assert!(valid);
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("hello", "hello"));
        assert!(!constant_time_eq("hello", "world"));
        assert!(!constant_time_eq("hello", "hello2"));
    }

    #[test]
    fn test_generate_api_key() {
        let (key, hash, prefix) = generate_api_key("tempo_live");
        
        assert!(key.starts_with("tempo_live_"));
        assert_eq!(prefix, "tempo_live");
        assert_eq!(hash.len(), 64); // SHA256 hex = 64 chars
        
        // Verify hash matches
        let verify_hash = hash_api_key(&key);
        assert_eq!(hash, verify_hash);
    }

    #[test]
    fn test_generate_webhook_secret() {
        let secret = generate_webhook_secret();
        assert!(secret.starts_with("whsec_"));
        assert_eq!(secret.len(), 71); // "whsec_" + 64 hex chars
    }
}
