//! Proof that an update came from us.
//!
//! Certificate validation authenticates whoever answered, and on the route most
//! mainland users take that is a ghproxy mirror — a third party that terminates
//! TLS legitimately, holds a valid certificate for its own name, and could serve
//! any bytes it liked with the handshake still perfect. The release listing
//! comes through the same mirror, so publishing a hash in it would only mean
//! asking the same party twice.
//!
//! A signature is the one thing a mirror cannot produce. The key that makes it
//! never leaves the release workflow; the key that checks it is compiled in
//! here, so what is verified is "we built this", not "someone with a valid
//! certificate handed it over".
//!
//! This is not code signing. Authenticode is a separate problem with a price
//! tag, and it is what Windows SmartScreen and Defender read; this is invisible
//! to them and answers a different question.

use base64::Engine;

use crate::core::error::UpdateError;

/// The public half of the key releases are signed with.
///
/// Base64 of a minisign public key file, as printed by
/// `npx @tauri-apps/cli signer generate`. Changing it invalidates every update
/// signed with the old key, so it changes only if the private half is lost or
/// compromised — and then the release before the change has to be installed by
/// hand, since no build after it will verify against what users already have.
const UPDATE_PUBLIC_KEY: &str = "";

/// Whether a public key has been published yet.
///
/// Until it has, an update cannot be shown to be ours, and the alternative to
/// refusing is running an unidentified executable — which is the thing this
/// exists to stop.
pub fn signing_configured() -> bool {
    !UPDATE_PUBLIC_KEY.is_empty()
}

/// Check `bytes` against `signature`, which is the content of the `.sig` file
/// published beside the release asset.
pub fn verify(bytes: &[u8], signature: &str) -> Result<(), UpdateError> {
    verify_with_key(bytes, signature, UPDATE_PUBLIC_KEY)
}

/// [`verify`] against a stated key, so the tests can exercise it with one whose
/// private half is a throwaway.
fn verify_with_key(bytes: &[u8], signature: &str, public_key: &str) -> Result<(), UpdateError> {
    let fail = |what: &str| UpdateError::DownloadFailed {
        reason: format!("the update could not be shown to be ours: {what}"),
    };

    if public_key.is_empty() {
        return Err(fail("no signing key is published"));
    }

    // Both halves are base64 around an ordinary minisign file, which is how the
    // Tauri signer writes them.
    let decode = |s: &str, what: &str| -> Result<String, UpdateError> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(s.trim())
            .map_err(|_| fail(&format!("the {what} is not base64")))?;
        String::from_utf8(raw).map_err(|_| fail(&format!("the {what} is not text")))
    };

    let key = minisign_verify::PublicKey::decode(&decode(public_key, "public key")?)
        .map_err(|e| fail(&format!("the public key is unusable ({e})")))?;
    let sig = minisign_verify::Signature::decode(&decode(signature, "signature")?)
        .map_err(|e| fail(&format!("the signature is unreadable ({e})")))?;

    key.verify(bytes, &sig, false)
        .map_err(|_| fail("the signature does not match these bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway keypair, generated for these tests alone. Its private half
    /// was never used for anything and is not kept — the point is only that
    /// these are a real minisign key and a real signature over known bytes.
    const TEST_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDhGM0IwMDEyMzZBNkU1RjYKUldUMjVhWTJFZ0E3andncEVhbVl0V2FTTlhkUzhMcVlXT3VqYy80Uis4QTB4VTlKb3E5eG02Qi8K";

    /// The signature that key produced over `TEST_BYTES`.
    const TEST_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUMjVhWTJFZ0E3ajdrc0tSVkN3UU9nYXdpT0c0TzdRWFhVQjg1QjBBTHMrRjBRWDN2MXNseit0Y3dmbFBPWDVVTUlLUStseUVMUG1xNUVVVmxHcDZNTmJNYzlrRHMyZkFRPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzg3ODQ4MDc4CWZpbGU6c2FtcGxlLmJpbgpudWdxaFRCSUNqcE82NGJNWWN0Z1NzWmdEY3Zoem9XdDI2d3RtbjlrRWk1TkNyWEEwbENCay9lUndQT2Uwd0pWaklHWVRqRFNHTDNUVGpZRG85ZXBDUT09Cg==";

    const TEST_BYTES: &[u8] = b"pretend this is MapleLink.exe";

    #[test]
    fn a_signature_over_these_bytes_verifies() {
        verify_with_key(TEST_BYTES, TEST_SIGNATURE, TEST_PUBLIC_KEY).unwrap();
    }

    /// The case this exists for: a mirror serves a different executable and the
    /// signature it was given no longer describes it.
    #[test]
    fn substituted_bytes_are_refused() {
        let swapped = b"pretend this is someone else's exe";
        let err = verify_with_key(swapped, TEST_SIGNATURE, TEST_PUBLIC_KEY).unwrap_err();
        assert!(err.to_string().contains("does not match"), "got: {err}");
    }

    /// A single flipped byte is enough.
    #[test]
    fn one_altered_byte_is_refused() {
        let mut tampered = TEST_BYTES.to_vec();
        tampered[0] ^= 0x01;
        verify_with_key(&tampered, TEST_SIGNATURE, TEST_PUBLIC_KEY).unwrap_err();
    }

    /// A mirror that forges a whole signature has to forge it under our key,
    /// which is the part it cannot do.
    #[test]
    fn a_signature_from_another_key_is_refused() {
        // Same shape, different key: the signer's own public key rather than
        // ours, which is what an attacker with a keypair would have.
        let other = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDBBMEEwQTBBMEEwQTBBMEEKUldRS0Nnb0tDZ29LQ2dvS0Nnb0tDZ29LQ2dvS0Nnb0tDZ29LQ2dvS0Nnb0tDZ29LQ2dvSwo=";
        verify_with_key(TEST_BYTES, TEST_SIGNATURE, other).unwrap_err();
    }

    #[test]
    fn rubbish_in_place_of_a_signature_is_refused() {
        for bad in ["", "not base64!!", "aGVsbG8="] {
            verify_with_key(TEST_BYTES, bad, TEST_PUBLIC_KEY).unwrap_err();
        }
    }

    /// Nothing verifies against an unset key — including, deliberately, a valid
    /// signature. Better a refused update than an unidentified one.
    #[test]
    fn an_unpublished_key_verifies_nothing() {
        assert!(verify_with_key(TEST_BYTES, TEST_SIGNATURE, "").is_err());
    }

    /// Fails until the key is published, which is the point: it is the one step
    /// that cannot be done from inside the repository, and a green build would
    /// hide that the protection is not on yet.
    #[test]
    #[ignore = "enable once UPDATE_PUBLIC_KEY is filled in"]
    fn the_shipped_key_is_a_real_one() {
        assert!(signing_configured(), "UPDATE_PUBLIC_KEY is still empty");
        // Wrong bytes, real key: reaching "does not match" proves the key
        // parsed, without needing a signature made by the private half.
        let err = verify(b"not the release", TEST_SIGNATURE).unwrap_err();
        assert!(
            err.to_string().contains("does not match"),
            "the shipped key did not parse: {err}"
        );
    }
}
