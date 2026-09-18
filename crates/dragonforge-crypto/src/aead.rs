use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};

use crate::{
    AeadCipher, CURRENT_ENVELOPE_VERSION, CipherSuite, CryptoError, EncryptedEnvelope,
    RandomSource, Result, SecretKey,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct Aes256GcmCipher;

impl AeadCipher for Aes256GcmCipher {
    fn seal(
        &self,
        random: &dyn RandomSource,
        key: &SecretKey,
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<EncryptedEnvelope> {
        let cipher =
            Aes256Gcm::new_from_slice(key.expose()).map_err(|_| CryptoError::EncryptionFailed)?;
        let mut nonce_bytes = [0_u8; 12];
        random.fill_bytes(&mut nonce_bytes)?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, Payload { msg: plaintext, aad })
            .map_err(|_| CryptoError::EncryptionFailed)?;

        Ok(EncryptedEnvelope::from_parts(
            CURRENT_ENVELOPE_VERSION,
            CipherSuite::Aes256GcmV1,
            nonce_bytes,
            ciphertext,
        ))
    }

    fn open(
        &self,
        key: &SecretKey,
        envelope: &EncryptedEnvelope,
        aad: &[u8],
    ) -> Result<Vec<u8>> {
        if envelope.version() != CURRENT_ENVELOPE_VERSION {
            return Err(CryptoError::UnsupportedEnvelopeVersion(envelope.version()));
        }
        if envelope.suite() != CipherSuite::Aes256GcmV1 {
            return Err(CryptoError::UnsupportedCipherSuite);
        }

        let cipher =
            Aes256Gcm::new_from_slice(key.expose()).map_err(|_| CryptoError::DecryptionFailed)?;
        let nonce_bytes = envelope.nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: envelope.ciphertext(),
                    aad,
                },
            )
            .map_err(|_| CryptoError::DecryptionFailed)
    }
}
