use serde::{Deserialize, Serialize};

pub const CURRENT_ENVELOPE_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CipherSuite {
    Aes256GcmV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    version: u16,
    suite: CipherSuite,
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

impl EncryptedEnvelope {
    #[must_use]
    pub fn from_parts(
        version: u16,
        suite: CipherSuite,
        nonce: [u8; 12],
        ciphertext: Vec<u8>,
    ) -> Self {
        Self {
            version,
            suite,
            nonce,
            ciphertext,
        }
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub const fn suite(&self) -> CipherSuite {
        self.suite
    }

    #[must_use]
    pub const fn nonce(&self) -> [u8; 12] {
        self.nonce
    }

    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }
}
