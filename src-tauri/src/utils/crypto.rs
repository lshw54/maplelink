//! DES ECB decryption for Beanfun OTP retrieval.
//!
//! DES ECB decryption matching the original WCDESComp.DecryStrHex:
//! DES ECB mode, no padding, ASCII key, hex-encoded ciphertext input.

use des::cipher::{BlockDecrypt, KeyInit};
use des::Des;

/// Decrypt a hex-encoded DES ECB ciphertext using an 8-byte ASCII key.
///
/// Returns the decrypted ASCII string with null bytes trimmed.
///
/// # Errors
///
/// Returns `None` if the key is not exactly 8 bytes, the hex string is
/// invalid, or the ciphertext length is not a multiple of 8 bytes.
pub fn des_ecb_decrypt_hex(hex_ciphertext: &str, key_ascii: &str) -> Option<String> {
    if key_ascii.len() != 8 {
        return None;
    }

    let key_bytes: [u8; 8] = key_ascii.as_bytes().try_into().ok()?;
    let cipher = Des::new_from_slice(&key_bytes).ok()?;

    let ciphertext = hex::decode(hex_ciphertext).ok()?;
    if ciphertext.len() % 8 != 0 {
        return None;
    }

    let mut plaintext = ciphertext;
    for chunk in plaintext.chunks_exact_mut(8) {
        let block = des::cipher::generic_array::GenericArray::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }

    // Trim null bytes and convert to ASCII string
    let trimmed: Vec<u8> = plaintext.into_iter().take_while(|&b| b != 0).collect();
    String::from_utf8(trimmed).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_known_value() {
        // Encrypt "HELLO123" with key "TESTKEY1" in DES ECB, no padding
        // We test round-trip by encrypting first
        use des::cipher::BlockEncrypt;

        let key = b"TESTKEY1";
        let plaintext = b"HELLO123"; // exactly 8 bytes
        let cipher = Des::new_from_slice(key).unwrap();

        let mut block = *plaintext;
        let ga = des::cipher::generic_array::GenericArray::from_mut_slice(&mut block);
        cipher.encrypt_block(ga);

        let hex_ct: String = block.iter().map(|b| format!("{b:02X}")).collect();
        let result = des_ecb_decrypt_hex(&hex_ct, "TESTKEY1").unwrap();
        assert_eq!(result, "HELLO123");
    }

    #[test]
    fn invalid_key_length_returns_none() {
        assert!(des_ecb_decrypt_hex("0000000000000000", "SHORT").is_none());
    }

    #[test]
    fn invalid_hex_returns_none() {
        assert!(des_ecb_decrypt_hex("ZZZZ", "TESTKEY1").is_none());
    }

    #[test]
    fn non_multiple_of_8_returns_none() {
        assert!(des_ecb_decrypt_hex("00000000000000", "TESTKEY1").is_none());
    }

    #[test]
    fn trims_null_bytes() {
        use des::cipher::BlockEncrypt;

        let key = b"TESTKEY1";
        // "HI" + 6 null bytes
        let mut plaintext = [0u8; 8];
        plaintext[0] = b'H';
        plaintext[1] = b'I';

        let cipher = Des::new_from_slice(key).unwrap();
        let ga = des::cipher::generic_array::GenericArray::from_mut_slice(&mut plaintext);
        cipher.encrypt_block(ga);

        let hex_ct: String = plaintext.iter().map(|b| format!("{b:02X}")).collect();
        let result = des_ecb_decrypt_hex(&hex_ct, "TESTKEY1").unwrap();
        assert_eq!(result, "HI");
    }
}
