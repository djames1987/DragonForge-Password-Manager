use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

pub fn new_sync_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[must_use]
pub fn hash_sync_token(token: &str) -> [u8; 32] {
    let digest = Sha256::digest(token.as_bytes());
    let mut result = [0_u8; 32];
    result.copy_from_slice(&digest);
    result
}
