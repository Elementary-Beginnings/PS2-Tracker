//! PKCE (RFC 7636, method S256) and the random `state`/`nonce` values of the
//! Discord sign-in.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

/// A fresh random value: 32 bytes from the operating system, base64url
/// without padding (43 characters, all in the RFC 7636 "unreserved" set).
pub fn random_token() -> Result<String, getrandom::Error> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b)?;
    Ok(URL_SAFE_NO_PAD.encode(b))
}

/// `code_challenge` for a verifier: BASE64URL(SHA256(ASCII(verifier))).
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Verifier and challenge of one sign-in. The verifier never leaves the
/// tracker except in the token exchange with the stats server.
#[derive(Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn new() -> Result<Self, getrandom::Error> {
        let verifier = random_token()?;
        Ok(Self {
            challenge: challenge(&verifier),
            verifier,
        })
    }
}

impl std::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pkce(***)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b() {
        // the example of RFC 7636, appendix B
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_shape() {
        let p = Pkce::new().unwrap();
        // 43..=128 characters of [A-Za-z0-9-._~]
        assert_eq!(p.verifier.len(), 43);
        assert!(
            p.verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
        );
        assert_eq!(p.challenge, challenge(&p.verifier));
        assert_ne!(Pkce::new().unwrap().verifier, p.verifier);
        assert!(!format!("{p:?}").contains(&p.verifier));
    }
}
