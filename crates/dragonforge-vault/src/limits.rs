//! Defensive limits for untrusted/local vault files.
//!
//! These limits bound memory allocation and parsing work before cryptographic
//! verification. They are format-policy limits, not cryptographic parameters.

pub const MAX_VAULT_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_VAULT_ITEMS: usize = 100_000;
pub const MAX_ITEM_CIPHERTEXT_BYTES: usize = 1024 * 1024;
pub const MAX_KDF_SALT_BYTES: usize = 64;
pub const MIN_AEAD_CIPHERTEXT_BYTES: usize = 16;
pub const WRAPPED_256_BIT_KEY_CIPHERTEXT_BYTES: usize = 32 + 16;
