//! Hashpool public key (hpub) encoding and decoding utilities
//!
//! This module provides functions for encoding and decoding secp256k1 public keys
//! in the hpub format, which uses bech32 encoding with the 'hpub' Human Readable Part (HRP).
//!
//! ## Format Specification
//!
//! - **Encoding**: Bech32 (BIP 173)
//! - **HRP**: `hpub` (identifies as hashpool locking pubkey)
//! - **Data**: 33-byte compressed secp256k1 public key (SEC1 format)
//!
//! ## Example
//!
//! ```text
//! hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7kxqz5p9
//! ^   ^                                             ^
//! |   |                                             |
//! |   +--- Bech32-encoded 33-byte pubkey -----------+
//! |
//! +-- Human Readable Part (identifies as hashpool pubkey)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use bitcoin::secp256k1::PublicKey;
//! use ehash::hpub::{encode_hpub, parse_hpub};
//!
//! // Encode a public key to hpub format
//! let hpub = encode_hpub(&pubkey)?;
//!
//! // Parse an hpub string back to a public key
//! let pubkey = parse_hpub(&hpub)?;
//! ```

use crate::error::MintError;
use bech32::{self, Bech32m, Hrp};
use bitcoin::secp256k1::PublicKey;

/// Parse an hpub-encoded string into a secp256k1 public key
///
/// This function validates the bech32 encoding, verifies the 'hpub' HRP,
/// checks the pubkey length (must be 33 bytes for compressed SEC1 format),
/// and returns the decoded public key.
///
/// # Arguments
///
/// * `hpub` - The bech32-encoded hpub string (e.g., "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k...")
///
/// # Returns
///
/// The decoded secp256k1 public key in compressed format
///
/// # Errors
///
/// Returns `MintError` if:
/// - The bech32 decoding fails
/// - The HRP is not 'hpub'
/// - The decoded data is not exactly 33 bytes
/// - The public key bytes are invalid
///
/// # Example
///
/// ```rust,ignore
/// let hpub = "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k...";
/// let pubkey = parse_hpub(hpub)?;
/// ```
pub fn parse_hpub(hpub: &str) -> Result<PublicKey, MintError> {
    // Decode the bech32 string - data will be Vec<u8> after bech32 decode
    let (hrp, bytes) = bech32::decode(hpub)
        .map_err(|e| MintError::ConfigError(format!("Failed to decode bech32: {}", e)))?;

    // Verify the HRP is 'hpub'
    let expected_hrp = Hrp::parse("hpub")
        .map_err(|e| MintError::ConfigError(format!("Invalid HRP: {}", e)))?;

    if hrp != expected_hrp {
        return Err(MintError::ConfigError(format!(
            "Invalid HRP: expected 'hpub', got '{}'",
            hrp
        )));
    }

    // Verify the length (must be 33 bytes for compressed SEC1 pubkey)
    if bytes.len() != 33 {
        return Err(MintError::ConfigError(format!(
            "Invalid pubkey length: expected 33 bytes, got {}",
            bytes.len()
        )));
    }

    // Parse as secp256k1 public key
    PublicKey::from_slice(&bytes)
        .map_err(|e| MintError::ConfigError(format!("Invalid public key: {}", e)))
}

/// Encode a secp256k1 public key to hpub format
///
/// This function serializes the public key to compressed SEC1 format (33 bytes)
/// and encodes it using bech32 with the 'hpub' HRP.
///
/// # Arguments
///
/// * `pubkey` - The secp256k1 public key to encode
///
/// # Returns
///
/// The bech32-encoded hpub string
///
/// # Errors
///
/// Returns `MintError` if bech32 encoding fails
///
/// # Example
///
/// ```rust,ignore
/// let hpub = encode_hpub(&pubkey)?;
/// println!("Encoded hpub: {}", hpub);
/// ```
pub fn encode_hpub(pubkey: &PublicKey) -> Result<String, MintError> {
    // Serialize pubkey to compressed SEC1 format (33 bytes)
    let pubkey_bytes = pubkey.serialize();

    // Create HRP
    let hrp = Hrp::parse("hpub")
        .map_err(|e| MintError::ConfigError(format!("Failed to create HRP: {}", e)))?;

    // Encode to bech32 - the library will handle the base32 conversion
    bech32::encode::<Bech32m>(hrp, &pubkey_bytes)
        .map_err(|e| MintError::ConfigError(format!("Failed to encode bech32: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};

    fn create_test_pubkey() -> PublicKey {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
        PublicKey::from_secret_key(&secp, &secret_key)
    }

    fn create_test_pubkey2() -> PublicKey {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[2u8; 32]).unwrap();
        PublicKey::from_secret_key(&secp, &secret_key)
    }

    #[test]
    fn test_encode_hpub() {
        let pubkey = create_test_pubkey();
        let result = encode_hpub(&pubkey);
        assert!(result.is_ok(), "encode_hpub should succeed");

        let hpub = result.unwrap();
        assert!(hpub.starts_with("hpub1"), "hpub should start with 'hpub1'");
        assert!(hpub.len() > 5, "hpub should have substantial length");
    }

    #[test]
    fn test_parse_hpub_roundtrip() {
        let original_pubkey = create_test_pubkey();

        // Encode to hpub
        let hpub = encode_hpub(&original_pubkey).unwrap();

        // Parse back
        let decoded_pubkey = parse_hpub(&hpub).unwrap();

        // Verify they match
        assert_eq!(
            original_pubkey.serialize(),
            decoded_pubkey.serialize(),
            "Roundtrip encoding/decoding should preserve pubkey"
        );
    }

    #[test]
    fn test_parse_hpub_invalid_hrp() {
        // Create a valid bech32 string with wrong HRP
        let pubkey = create_test_pubkey();
        let pubkey_bytes = pubkey.serialize();

        let wrong_hrp = Hrp::parse("test").unwrap();
        let wrong_hpub = bech32::encode::<Bech32m>(wrong_hrp, &pubkey_bytes).unwrap();

        // Should fail with invalid HRP error
        let result = parse_hpub(&wrong_hpub);
        assert!(result.is_err(), "parse_hpub should fail with invalid HRP");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid HRP"),
            "Error should mention invalid HRP, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_parse_hpub_invalid_length() {
        // Create bech32 with wrong length (only 32 bytes instead of 33)
        let short_bytes = [1u8; 32];

        let hrp = Hrp::parse("hpub").unwrap();
        let short_hpub = bech32::encode::<Bech32m>(hrp, &short_bytes).unwrap();

        // Should fail with invalid length error
        let result = parse_hpub(&short_hpub);
        assert!(result.is_err(), "parse_hpub should fail with invalid length");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid pubkey length"),
            "Error should mention invalid length, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_parse_hpub_invalid_bech32() {
        // Invalid bech32 string (bad checksum)
        let invalid_hpub = "hpub1invalid";

        let result = parse_hpub(invalid_hpub);
        assert!(result.is_err(), "parse_hpub should fail with invalid bech32");
    }

    #[test]
    fn test_encode_different_pubkeys() {
        let pubkey1 = create_test_pubkey();
        let pubkey2 = create_test_pubkey2();

        let hpub1 = encode_hpub(&pubkey1).unwrap();
        let hpub2 = encode_hpub(&pubkey2).unwrap();

        // Different pubkeys should produce different hpubs
        assert_ne!(
            hpub1, hpub2,
            "Different pubkeys should produce different hpubs"
        );
    }

    #[test]
    fn test_parse_multiple_different_hpubs() {
        let pubkey1 = create_test_pubkey();
        let pubkey2 = create_test_pubkey2();

        let hpub1 = encode_hpub(&pubkey1).unwrap();
        let hpub2 = encode_hpub(&pubkey2).unwrap();

        let decoded1 = parse_hpub(&hpub1).unwrap();
        let decoded2 = parse_hpub(&hpub2).unwrap();

        // Verify each decodes correctly
        assert_eq!(pubkey1.serialize(), decoded1.serialize());
        assert_eq!(pubkey2.serialize(), decoded2.serialize());

        // Verify they're different
        assert_ne!(decoded1.serialize(), decoded2.serialize());
    }

    #[test]
    fn test_hpub_format_properties() {
        let pubkey = create_test_pubkey();
        let hpub = encode_hpub(&pubkey).unwrap();

        // Verify format properties
        assert!(hpub.starts_with("hpub1"), "hpub should start with 'hpub1'");
        assert!(hpub.is_ascii(), "hpub should be ASCII");
        assert!(
            hpub.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "hpub should only contain lowercase alphanumeric characters"
        );
    }

    #[test]
    fn test_parse_hpub_invalid_pubkey_bytes() {
        // Create bech32 with invalid pubkey bytes (all zeros won't be a valid compressed pubkey)
        let invalid_bytes = [0u8; 33];

        let hrp = Hrp::parse("hpub").unwrap();
        let invalid_hpub = bech32::encode::<Bech32m>(hrp, &invalid_bytes).unwrap();

        // Should fail with invalid public key error
        let result = parse_hpub(&invalid_hpub);
        assert!(result.is_err(), "parse_hpub should fail with invalid pubkey bytes");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid public key"),
            "Error should mention invalid public key, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_encode_hpub_deterministic() {
        let pubkey = create_test_pubkey();

        // Encode multiple times
        let hpub1 = encode_hpub(&pubkey).unwrap();
        let hpub2 = encode_hpub(&pubkey).unwrap();

        // Should produce the same result every time
        assert_eq!(
            hpub1, hpub2,
            "Encoding the same pubkey should be deterministic"
        );
    }
}
