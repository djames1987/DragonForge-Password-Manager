use rand_core::{OsRng, RngCore};

use crate::{CryptoError, RandomSource, Result, SECRET_KEY_LEN, SecretKey};

#[derive(Debug, Default, Clone, Copy)]
pub struct OsRandom;

impl RandomSource for OsRandom {
    fn fill_bytes(&self, destination: &mut [u8]) -> Result<()> {
        let mut rng = OsRng;
        rng.try_fill_bytes(destination)
            .map_err(|_| CryptoError::RandomnessUnavailable)
    }
}

pub fn generate_secret_key(random: &dyn RandomSource) -> Result<SecretKey> {
    let mut bytes = [0_u8; SECRET_KEY_LEN];
    random.fill_bytes(&mut bytes)?;
    Ok(SecretKey::from_bytes(bytes))
}

pub fn generate_salt(random: &dyn RandomSource, length: usize) -> Result<Vec<u8>> {
    let mut salt = vec![0_u8; length];
    random.fill_bytes(&mut salt)?;
    Ok(salt)
}
