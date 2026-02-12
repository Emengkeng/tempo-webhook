use crate::error::{AppError, AppResult};
use regex::Regex;

lazy_static::lazy_static! {
    static ref EMAIL_REGEX: Regex = Regex::new(
        r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
    ).unwrap();
    
    static ref ETH_ADDRESS_REGEX: Regex = Regex::new(
        r"^0x[a-fA-F0-9]{40}$"
    ).unwrap();
    
    static ref URL_REGEX: Regex = Regex::new(
        r"^https?://[^\s/$.?#].[^\s]*$"
    ).unwrap();
}

pub fn validate_email(email: &str) -> AppResult<()> {
    if !EMAIL_REGEX.is_match(email) {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }
    Ok(())
}

pub fn validate_ethereum_address(address: &str) -> AppResult<()> {
    if !ETH_ADDRESS_REGEX.is_match(address) {
        return Err(AppError::BadRequest(
            "Invalid Ethereum address format".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_url(url: &str) -> AppResult<()> {
    if !URL_REGEX.is_match(url) {
        return Err(AppError::BadRequest("Invalid URL format".to_string()));
    }
    Ok(())
}

pub fn validate_network(network: &str) -> AppResult<()> {
    match network {
        "mainnet" | "testnet" => Ok(()),
        _ => Err(AppError::BadRequest(
            "Invalid network. Must be 'mainnet' or 'testnet'".to_string(),
        )),
    }
}

pub fn validate_event_type(event_type: &str) -> AppResult<()> {
    match event_type {
        "TRANSFER" | "TRANSFER_WITH_MEMO" | "MINT" | "BURN" | "SWAP" => Ok(()),
        _ => Err(AppError::BadRequest(format!(
            "Invalid event type: {}",
            event_type
        ))),
    }
}

pub fn validate_filter_type(filter_type: &str) -> AppResult<()> {
    match filter_type {
        "amount_min" | "amount_max" | "from_address" | "to_address" | "memo_pattern" => Ok(()),
        _ => Err(AppError::BadRequest(format!(
            "Invalid filter type: {}",
            filter_type
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("user+tag@domain.co.uk").is_ok());
        assert!(validate_email("invalid@").is_err());
        assert!(validate_email("@example.com").is_err());
    }

    #[test]
    fn test_validate_ethereum_address() {
        assert!(validate_ethereum_address("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb").is_ok());
        assert!(validate_ethereum_address("0xABCDEF0123456789ABCDEF0123456789ABCDEF01").is_ok());
        assert!(validate_ethereum_address("0x123").is_err());
        assert!(validate_ethereum_address("742d35Cc6634C0532925a3b844Bc9e7595f0bEb").is_err());
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com/webhook").is_ok());
        assert!(validate_url("http://localhost:3000/hooks").is_ok());
        assert!(validate_url("not-a-url").is_err());
    }

    #[test]
    fn test_validate_network() {
        assert!(validate_network("mainnet").is_ok());
        assert!(validate_network("testnet").is_ok());
        assert!(validate_network("invalid").is_err());
    }
}
