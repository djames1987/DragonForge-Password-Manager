use core::fmt;

use ml_kem::{
    KeyExport, KeyInit, MlKem768, TryKeyInit,
    kem::{Decapsulate, Encapsulate, Kem},
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{CryptoError, Result, SecretKey};

pub const ML_KEM_768_PUBLIC_KEY_LEN: usize = 1184;
pub const ML_KEM_768_PRIVATE_SEED_LEN: usize = 64;
pub const ML_KEM_768_CIPHERTEXT_LEN: usize = 1088;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlKem768PublicKey(Vec<u8>);

impl MlKem768PublicKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ML_KEM_768_PUBLIC_KEY_LEN {
            return Err(CryptoError::InvalidMlKemPublicKey);
        }
        let _ = ml_kem::ml_kem_768::EncapsulationKey::new_from_slice(bytes)
            .map_err(|_| CryptoError::InvalidMlKemPublicKey)?;
        Ok(Self(bytes.to_vec()))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for MlKem768PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MlKem768PublicKey")
            .field("len", &self.0.len())
            .finish()
    }
}

pub struct MlKem768KeyPair {
    secret: ml_kem::ml_kem_768::DecapsulationKey,
    public: MlKem768PublicKey,
}

impl MlKem768KeyPair {
    #[must_use]
    pub fn generate() -> Self {
        let (secret, public) = MlKem768::generate_keypair();
        Self {
            public: MlKem768PublicKey(public.to_bytes().to_vec()),
            secret,
        }
    }

    pub fn from_seed(seed: &[u8]) -> Result<Self> {
        if seed.len() != ML_KEM_768_PRIVATE_SEED_LEN {
            return Err(CryptoError::InvalidMlKemPrivateSeed);
        }
        let secret = ml_kem::ml_kem_768::DecapsulationKey::new_from_slice(seed)
            .map_err(|_| CryptoError::InvalidMlKemPrivateSeed)?;
        let public = MlKem768PublicKey(secret.encapsulation_key().to_bytes().to_vec());
        Ok(Self { secret, public })
    }

    #[must_use]
    pub fn public_key(&self) -> &MlKem768PublicKey {
        &self.public
    }

    #[must_use]
    pub fn export_seed(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.secret.to_bytes().to_vec())
    }

    pub fn decapsulate(&self, ciphertext: &[u8]) -> Result<SecretKey> {
        if ciphertext.len() != ML_KEM_768_CIPHERTEXT_LEN {
            return Err(CryptoError::InvalidMlKemCiphertext);
        }
        let shared = self
            .secret
            .decapsulate_slice(ciphertext)
            .map_err(|_| CryptoError::InvalidMlKemCiphertext)?;
        SecretKey::try_from_slice(shared.as_ref())
    }
}

impl fmt::Debug for MlKem768KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MlKem768KeyPair")
            .field("secret", &"[REDACTED]")
            .field("public", &self.public)
            .finish()
    }
}

pub fn ml_kem_768_encapsulate(public_key: &MlKem768PublicKey) -> Result<(Vec<u8>, SecretKey)> {
    let public = ml_kem::ml_kem_768::EncapsulationKey::new_from_slice(public_key.as_bytes())
        .map_err(|_| CryptoError::InvalidMlKemPublicKey)?;
    let (ciphertext, shared) = public.encapsulate();
    let key = SecretKey::try_from_slice(shared.as_ref())?;
    Ok((ciphertext.to_vec(), key))
}
