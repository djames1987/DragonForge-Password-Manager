use core::fmt;

use ml_dsa::{
    KeyExport, KeyInit, MlDsa65, Signature, SignatureEncoding, Signer, SigningKey, Verifier,
    VerifyingKey,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{CryptoError, Result};

pub const ML_DSA_65_PRIVATE_SEED_LEN: usize = 32;
pub const ML_DSA_65_PUBLIC_KEY_LEN: usize = 1952;
pub const ML_DSA_65_SIGNATURE_LEN: usize = 3309;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlDsa65VerifyingKey(Vec<u8>);

impl MlDsa65VerifyingKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ML_DSA_65_PUBLIC_KEY_LEN {
            return Err(CryptoError::InvalidMlDsaVerifyingKey);
        }
        let _ = VerifyingKey::<MlDsa65>::new_from_slice(bytes)
            .map_err(|_| CryptoError::InvalidMlDsaVerifyingKey)?;
        Ok(Self(bytes.to_vec()))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<()> {
        let verifying = VerifyingKey::<MlDsa65>::new_from_slice(&self.0)
            .map_err(|_| CryptoError::InvalidMlDsaVerifyingKey)?;
        let signature = Signature::<MlDsa65>::try_from(signature)
            .map_err(|_| CryptoError::InvalidMlDsaSignature)?;
        verifying
            .verify(message, &signature)
            .map_err(|_| CryptoError::SignatureVerificationFailed)
    }
}

impl fmt::Debug for MlDsa65VerifyingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MlDsa65VerifyingKey")
            .field("len", &self.0.len())
            .finish()
    }
}

pub struct MlDsa65KeyPair {
    signing: SigningKey<MlDsa65>,
    verifying: MlDsa65VerifyingKey,
}

impl MlDsa65KeyPair {
    #[must_use]
    pub fn generate() -> Self {
        let signing = SigningKey::<MlDsa65>::generate();
        let verifying = MlDsa65VerifyingKey(signing.verifying_key().to_bytes().to_vec());
        Self { signing, verifying }
    }

    pub fn from_seed(seed: &[u8]) -> Result<Self> {
        if seed.len() != ML_DSA_65_PRIVATE_SEED_LEN {
            return Err(CryptoError::InvalidMlDsaPrivateSeed);
        }
        let signing = SigningKey::<MlDsa65>::new_from_slice(seed)
            .map_err(|_| CryptoError::InvalidMlDsaPrivateSeed)?;
        let verifying = MlDsa65VerifyingKey(signing.verifying_key().to_bytes().to_vec());
        Ok(Self { signing, verifying })
    }

    #[must_use]
    pub fn verifying_key(&self) -> &MlDsa65VerifyingKey {
        &self.verifying
    }

    #[must_use]
    pub fn export_seed(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.signing.to_bytes().to_vec())
    }

    #[must_use]
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let signature: Signature<MlDsa65> = self.signing.sign(message);
        signature.to_vec()
    }
}

impl fmt::Debug for MlDsa65KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MlDsa65KeyPair")
            .field("signing", &"[REDACTED]")
            .field("verifying", &self.verifying)
            .finish()
    }
}
