#![forbid(unsafe_code)]
//! Security-focused cryptographic building blocks for DragonForge Password Manager.

mod aead;
mod ct;
mod derive;
mod envelope;
mod error;
mod hybrid;
mod kdf;
mod keys;
mod pq_kem;
mod pq_sign;
mod random;
mod traits;
mod wrap;

pub use aead::Aes256GcmCipher;
pub use ct::constant_time_eq;
pub use derive::HkdfSha512;
pub use envelope::{CURRENT_ENVELOPE_VERSION, CipherSuite, EncryptedEnvelope};
pub use error::{CryptoError, Result};
pub use hybrid::{
    HybridKemCiphertext, HybridKemKeyPair, HybridKemPublicKey, hybrid_kem_encapsulate,
};
pub use kdf::{
    Argon2idConfig, Argon2idKdf, MIN_ITERATIONS, MIN_LANES, MIN_MEMORY_KIB, MIN_SALT_LEN,
};
pub use keys::{SECRET_KEY_LEN, SecretKey};
pub use pq_kem::{
    ML_KEM_768_CIPHERTEXT_LEN, ML_KEM_768_PRIVATE_SEED_LEN, ML_KEM_768_PUBLIC_KEY_LEN,
    MlKem768KeyPair, MlKem768PublicKey, ml_kem_768_encapsulate,
};
pub use pq_sign::{
    ML_DSA_65_PRIVATE_SEED_LEN, ML_DSA_65_PUBLIC_KEY_LEN, ML_DSA_65_SIGNATURE_LEN, MlDsa65KeyPair,
    MlDsa65VerifyingKey,
};
pub use random::{OsRandom, generate_salt, generate_secret_key};
pub use traits::{AeadCipher, KeyDeriver, PasswordKdf, RandomSource};
pub use wrap::{unwrap_key, wrap_key};
