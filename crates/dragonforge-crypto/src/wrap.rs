use crate::{AeadCipher, CryptoError, EncryptedEnvelope, RandomSource, Result, SecretKey};

const KEY_WRAP_DOMAIN: &[u8] = b"dragonforge/key-wrap/v1";

fn key_wrap_aad(context: &[u8]) -> Result<Vec<u8>> {
    if context.is_empty() {
        return Err(CryptoError::EmptyContext);
    }

    let context_len = u64::try_from(context.len()).map_err(|_| CryptoError::EmptyContext)?;
    let mut aad = Vec::with_capacity(KEY_WRAP_DOMAIN.len() + 8 + context.len());
    aad.extend_from_slice(KEY_WRAP_DOMAIN);
    aad.extend_from_slice(&context_len.to_be_bytes());
    aad.extend_from_slice(context);
    Ok(aad)
}

pub fn wrap_key(
    cipher: &dyn AeadCipher,
    random: &dyn RandomSource,
    wrapping_key: &SecretKey,
    key_to_wrap: &SecretKey,
    context: &[u8],
) -> Result<EncryptedEnvelope> {
    let aad = key_wrap_aad(context)?;
    cipher.seal(random, wrapping_key, key_to_wrap.expose(), &aad)
}

pub fn unwrap_key(
    cipher: &dyn AeadCipher,
    wrapping_key: &SecretKey,
    envelope: &EncryptedEnvelope,
    context: &[u8],
) -> Result<SecretKey> {
    let aad = key_wrap_aad(context)?;
    let plaintext = cipher.open(wrapping_key, envelope, &aad)?;
    SecretKey::try_from_slice(&plaintext)
}
