use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("invalid key length: expected {expected} bytes, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
    #[error("salt is too short: minimum {minimum} bytes, got {actual}")]
    InvalidSaltLength { minimum: usize, actual: usize },
    #[error("invalid Argon2id parameters")]
    InvalidKdfParameters,
    #[error("cryptographic context must not be empty")]
    EmptyContext,
    #[error("operating-system randomness is unavailable")]
    RandomnessUnavailable,
    #[error("authenticated encryption failed")]
    EncryptionFailed,
    #[error("authenticated decryption failed")]
    DecryptionFailed,
    #[error("unsupported encrypted-envelope version: {0}")]
    UnsupportedEnvelopeVersion(u16),
    #[error("unsupported cipher suite")]
    UnsupportedCipherSuite,
    #[error("HKDF expansion failed")]
    HkdfExpandFailed,
    #[error("invalid ML-KEM-768 public key")]
    InvalidMlKemPublicKey,
    #[error("invalid ML-KEM-768 private seed")]
    InvalidMlKemPrivateSeed,
    #[error("invalid ML-KEM-768 ciphertext")]
    InvalidMlKemCiphertext,
    #[error("invalid ML-DSA-65 verifying key")]
    InvalidMlDsaVerifyingKey,
    #[error("invalid ML-DSA-65 private seed")]
    InvalidMlDsaPrivateSeed,
    #[error("invalid ML-DSA-65 signature")]
    InvalidMlDsaSignature,
    #[error("ML-DSA-65 signature verification failed")]
    SignatureVerificationFailed,
    #[error("X25519 peer public key produced a non-contributory shared secret")]
    NonContributoryX25519,
    #[error("invalid hybrid KEM material")]
    InvalidHybridMaterial,
}

pub type Result<T> = std::result::Result<T, CryptoError>;
