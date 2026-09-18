#![forbid(unsafe_code)]
//! Security-focused cryptographic building blocks for DragonForge Password Manager.

mod aead;
mod ct;
mod derive;
mod envelope;
mod error;
mod kdf;
mod keys;
mod random;
mod traits;
mod wrap;

pub use aead::Aes256GcmCipher;
pub use ct::constant_time_eq;
pub use derive::HkdfSha512;
pub use envelope::{CURRENT_ENVELOPE_VERSION, CipherSuite, EncryptedEnvelope};
pub use error::{CryptoError, Result};
pub use kdf::{
    Argon2idConfig, Argon2idKdf, MIN_ITERATIONS, MIN_LANES, MIN_MEMORY_KIB, MIN_SALT_LEN,
};
pub use keys::{SECRET_KEY_LEN, SecretKey};
pub use random::{OsRandom, generate_salt, generate_secret_key};
pub use traits::{AeadCipher, KeyDeriver, PasswordKdf, RandomSource};
pub use wrap::{unwrap_key, wrap_key};
