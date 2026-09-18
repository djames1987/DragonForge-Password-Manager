use core::fmt;

use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha512;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::{
    CryptoError, MlKem768KeyPair, MlKem768PublicKey, Result, SECRET_KEY_LEN, SecretKey,
    ml_kem_768_encapsulate,
};

const HYBRID_DOMAIN: &[u8] = b"dragonforge/hybrid/x25519-mlkem768/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridKemPublicKey {
    pub x25519: [u8; 32],
    pub ml_kem_768: MlKem768PublicKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridKemCiphertext {
    pub ephemeral_x25519: [u8; 32],
    pub ml_kem_768_ciphertext: Vec<u8>,
}

pub struct HybridKemKeyPair {
    x25519_secret: StaticSecret,
    ml_kem: MlKem768KeyPair,
    public: HybridKemPublicKey,
}

impl HybridKemKeyPair {
    #[must_use]
    pub fn generate() -> Self {
        let x25519_secret = StaticSecret::random();
        let x25519_public = PublicKey::from(&x25519_secret).to_bytes();
        let ml_kem = MlKem768KeyPair::generate();
        let public = HybridKemPublicKey {
            x25519: x25519_public,
            ml_kem_768: ml_kem.public_key().clone(),
        };
        Self {
            x25519_secret,
            ml_kem,
            public,
        }
    }

    #[must_use]
    pub fn public_key(&self) -> &HybridKemPublicKey {
        &self.public
    }

    pub fn decapsulate(
        &self,
        ciphertext: &HybridKemCiphertext,
        context: &[u8],
    ) -> Result<SecretKey> {
        if context.is_empty() {
            return Err(CryptoError::EmptyContext);
        }

        let sender_public = PublicKey::from(ciphertext.ephemeral_x25519);
        let classical = self.x25519_secret.diffie_hellman(&sender_public);
        if !classical.was_contributory() {
            return Err(CryptoError::NonContributoryX25519);
        }

        let pq = self.ml_kem.decapsulate(&ciphertext.ml_kem_768_ciphertext)?;
        combine_hybrid_secrets(classical.as_bytes(), &pq, &self.public, ciphertext, context)
    }
}

impl fmt::Debug for HybridKemKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridKemKeyPair")
            .field("x25519_secret", &"[REDACTED]")
            .field("ml_kem", &"[REDACTED]")
            .field("public", &self.public)
            .finish()
    }
}

pub fn hybrid_kem_encapsulate(
    recipient: &HybridKemPublicKey,
    context: &[u8],
) -> Result<(HybridKemCiphertext, SecretKey)> {
    if context.is_empty() {
        return Err(CryptoError::EmptyContext);
    }

    let recipient_x25519 = PublicKey::from(recipient.x25519);
    let ephemeral = EphemeralSecret::random();
    let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
    let classical = ephemeral.diffie_hellman(&recipient_x25519);
    if !classical.was_contributory() {
        return Err(CryptoError::NonContributoryX25519);
    }

    let (pq_ciphertext, pq) = ml_kem_768_encapsulate(&recipient.ml_kem_768)?;
    let ciphertext = HybridKemCiphertext {
        ephemeral_x25519: ephemeral_public,
        ml_kem_768_ciphertext: pq_ciphertext,
    };

    let key = combine_hybrid_secrets(classical.as_bytes(), &pq, recipient, &ciphertext, context)?;
    Ok((ciphertext, key))
}

fn combine_hybrid_secrets(
    classical: &[u8; 32],
    pq: &SecretKey,
    recipient: &HybridKemPublicKey,
    ciphertext: &HybridKemCiphertext,
    context: &[u8],
) -> Result<SecretKey> {
    let mut ikm = Zeroizing::new(Vec::with_capacity(64));
    ikm.extend_from_slice(classical);
    ikm.extend_from_slice(pq.expose());

    let mut info = Vec::with_capacity(
        HYBRID_DOMAIN.len()
            + context.len()
            + recipient.x25519.len()
            + recipient.ml_kem_768.as_bytes().len()
            + ciphertext.ephemeral_x25519.len()
            + ciphertext.ml_kem_768_ciphertext.len()
            + 32,
    );
    append_field(&mut info, HYBRID_DOMAIN);
    append_field(&mut info, context);
    append_field(&mut info, &recipient.x25519);
    append_field(&mut info, recipient.ml_kem_768.as_bytes());
    append_field(&mut info, &ciphertext.ephemeral_x25519);
    append_field(&mut info, &ciphertext.ml_kem_768_ciphertext);

    let hkdf = Hkdf::<Sha512>::new(None, &ikm);
    let mut output = Zeroizing::new([0_u8; SECRET_KEY_LEN]);
    hkdf.expand(&info, output.as_mut())
        .map_err(|_| CryptoError::HkdfExpandFailed)?;
    Ok(SecretKey::from_bytes(*output))
}

fn append_field(destination: &mut Vec<u8>, field: &[u8]) {
    destination.extend_from_slice(&(field.len() as u64).to_be_bytes());
    destination.extend_from_slice(field);
}
