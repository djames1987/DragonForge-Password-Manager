use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::{CryptoError, PasswordKdf, Result, SECRET_KEY_LEN, SecretKey};

pub const MIN_SALT_LEN: usize = 16;
pub const MIN_MEMORY_KIB: u32 = 19 * 1024;
pub const MIN_ITERATIONS: u32 = 2;
pub const MIN_LANES: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2idConfig {
    pub memory_kib: u32,
    pub iterations: u32,
    pub lanes: u32,
}

impl Argon2idConfig {
    pub fn new(memory_kib: u32, iterations: u32, lanes: u32) -> Result<Self> {
        let config = Self {
            memory_kib,
            iterations,
            lanes,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(self) -> Result<()> {
        if self.memory_kib < MIN_MEMORY_KIB
            || self.iterations < MIN_ITERATIONS
            || self.lanes < MIN_LANES
        {
            return Err(CryptoError::InvalidKdfParameters);
        }

        Params::new(
            self.memory_kib,
            self.iterations,
            self.lanes,
            Some(SECRET_KEY_LEN),
        )
        .map_err(|_| CryptoError::InvalidKdfParameters)?;
        Ok(())
    }
}

impl Default for Argon2idConfig {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            lanes: 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Argon2idKdf {
    config: Argon2idConfig,
}

impl Argon2idKdf {
    pub fn new(config: Argon2idConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    #[must_use]
    pub const fn config(&self) -> Argon2idConfig {
        self.config
    }
}

impl Default for Argon2idKdf {
    fn default() -> Self {
        Self {
            config: Argon2idConfig::default(),
        }
    }
}

impl PasswordKdf for Argon2idKdf {
    fn derive_key(&self, password: &[u8], salt: &[u8]) -> Result<SecretKey> {
        self.config.validate()?;
        if salt.len() < MIN_SALT_LEN {
            return Err(CryptoError::InvalidSaltLength {
                minimum: MIN_SALT_LEN,
                actual: salt.len(),
            });
        }

        let params = Params::new(
            self.config.memory_kib,
            self.config.iterations,
            self.config.lanes,
            Some(SECRET_KEY_LEN),
        )
        .map_err(|_| CryptoError::InvalidKdfParameters)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut output = Zeroizing::new([0_u8; SECRET_KEY_LEN]);
        argon2
            .hash_password_into(password, salt, output.as_mut())
            .map_err(|_| CryptoError::InvalidKdfParameters)?;

        Ok(SecretKey::from_bytes(*output))
    }
}
