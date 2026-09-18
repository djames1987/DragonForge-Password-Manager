use core::fmt;
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::{CryptoError, Result};

pub const SECRET_KEY_LEN: usize = 32;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretKey([u8; SECRET_KEY_LEN]);

impl SecretKey {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; SECRET_KEY_LEN]) -> Self {
        Self(bytes)
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self> {
        let actual = bytes.len();
        let array: [u8; SECRET_KEY_LEN] = bytes.try_into().map_err(|_| {
            CryptoError::InvalidKeyLength {
                expected: SECRET_KEY_LEN,
                actual,
            }
        })?;
        Ok(Self(array))
    }

    #[must_use]
    pub fn ct_eq(&self, other: &Self) -> bool {
        bool::from(self.0.ct_eq(&other.0))
    }

    pub(crate) const fn expose(&self) -> &[u8; SECRET_KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretKey([REDACTED])")
    }
}
