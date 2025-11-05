//! eHash work calculation functions
//!
//! This module implements the hashpool eHash calculation algorithm:
//! - Counts leading zero bits in share hashes
//! - Calculates exponential eHash amounts based on difficulty

/// Calculate the number of leading zero bits in a hash
///
/// Counts leading zero bits across all 32 bytes, stopping at the first non-zero bit.
///
/// # Arguments
/// * `hash` - The 32-byte share hash
///
/// # Returns
/// The number of leading zero bits (0-256)
pub fn calculate_difficulty(hash: [u8; 32]) -> u32 {
    let mut leading_zeros = 0u32;

    for byte in hash.iter() {
        if *byte == 0 {
            leading_zeros += 8;
        } else {
            // Count leading zeros in this byte and stop
            leading_zeros += byte.leading_zeros();
            break;
        }
    }

    leading_zeros
}

/// Calculate eHash amount for a share hash by counting leading hex zeros
///
/// Formula: Count the number of leading '0' hex characters in the hash
///
/// # Arguments
/// * `hash` - The 32-byte share hash (big-endian format)
/// * `min_leading_zeros` - Minimum leading hex zeros required to earn any eHash
///
/// # Returns
/// The number of leading hex zeros (0 if below minimum threshold)
///
/// # Examples
/// ```
/// use ehash_integration::calculate_ehash_amount;
///
/// // Hash with 12 leading hex zeros
/// let mut hash = [0xffu8; 32];
/// hash[..6].fill(0x00); // 6 bytes = 12 hex zeros
/// let amount = calculate_ehash_amount(hash, 10);
/// // Returns 12 (the count of leading hex zeros)
/// ```
pub fn calculate_ehash_amount(hash: [u8; 32], min_leading_zeros: u32) -> u64 {
    // Count leading hex zeros (each byte = 2 hex chars)
    let mut leading_hex_zeros = 0u32;

    for byte in hash.iter() {
        if *byte == 0 {
            leading_hex_zeros += 2; // Each zero byte = 2 hex zeros
        } else if *byte < 0x10 {
            leading_hex_zeros += 1; // High nibble is zero
            break;
        } else {
            break;
        }
    }

    // If below minimum threshold, return 0
    if leading_hex_zeros < min_leading_zeros {
        return 0;
    }

    leading_hex_zeros as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_difficulty_all_zeros() {
        let hash = [0u8; 32];
        assert_eq!(calculate_difficulty(hash), 256);
    }

    #[test]
    fn test_calculate_difficulty_first_byte_nonzero() {
        let mut hash = [0u8; 32];
        hash[0] = 0b10000000; // 0 leading zeros
        assert_eq!(calculate_difficulty(hash), 0);

        hash[0] = 0b01000000; // 1 leading zero
        assert_eq!(calculate_difficulty(hash), 1);

        hash[0] = 0b00000001; // 7 leading zeros
        assert_eq!(calculate_difficulty(hash), 7);
    }

    #[test]
    fn test_calculate_difficulty_second_byte_nonzero() {
        let mut hash = [0u8; 32];
        hash[1] = 0b10000000; // 8 leading zeros (first byte all zeros)
        assert_eq!(calculate_difficulty(hash), 8);

        hash[1] = 0b00000001; // 15 leading zeros
        assert_eq!(calculate_difficulty(hash), 15);
    }

    #[test]
    fn test_calculate_ehash_amount_below_minimum() {
        let mut hash = [0u8; 32];
        hash[0] = 0xff; // 0 leading zeros
        assert_eq!(calculate_ehash_amount(hash, 32), 0);
    }

    #[test]
    fn test_calculate_ehash_amount_at_minimum() {
        // 32 leading zeros
        let mut hash = [0u8; 32];
        hash[3] = 0xff; // First 24 bits are zero
        hash[4] = 0xff; // Byte 4 has some non-zero bits
        // Actually this gives us 24 leading zeros, let me recalculate
        // We need exactly 32 leading zeros = 4 bytes of zeros
        hash = [0u8; 32];
        hash[4] = 0xff; // First 32 bits (4 bytes) are zero

        assert_eq!(calculate_ehash_amount(hash, 32), 1); // 2^(32-32) = 2^0 = 1
    }

    #[test]
    fn test_calculate_ehash_amount_above_minimum() {
        // 40 leading zeros
        let mut hash = [0u8; 32];
        hash[5] = 0xff; // First 40 bits (5 bytes) are zero

        assert_eq!(calculate_ehash_amount(hash, 32), 256); // 2^(40-32) = 2^8 = 256
    }

    #[test]
    fn test_calculate_ehash_amount_capped() {
        let hash = [0u8; 32]; // 256 leading zeros

        // Should be capped at 2^63
        assert_eq!(calculate_ehash_amount(hash, 32), 1u64 << 63);
        assert_eq!(calculate_ehash_amount(hash, 0), 1u64 << 63);
    }
}
