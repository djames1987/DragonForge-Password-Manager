use hkdf::Hkdf;
use sha2::Sha512;
use zeroize::Zeroizing;

use crate::{CryptoError, KeyDeriver, Result, SECRET_KEY_LEN, SecretKey};

#[derive(Debug, Default, Clone, Copy)]
pub struct HkdfSha512;

impl KeyDeriver for HkdfSha512 {
    fn derive_key(
        &self,
        input_key: &SecretKey,
        salt: Option<&[u8]>,
        info: &[u8],
    ) -> Result<SecretKey> {
        if info.is_empty() {
            return Err(CryptoError::EmptyContext);
        }

        let hkdf = Hkdf::<Sha512>::new(salt, input_key.expose());
        let mut output = Zeroizing::new([0_u8; SECRET_KEY_LEN]);
        hkdf.expand(info, output.as_mut())
            .map_err(|_| CryptoError::HkdfExpandFailed)?;
        Ok(SecretKey::from_bytes(*output))
    }
}
