use crate::{EncryptedEnvelope, Result, SecretKey};

pub trait PasswordKdf {
    fn derive_key(&self, password: &[u8], salt: &[u8]) -> Result<SecretKey>;
}

pub trait AeadCipher {
    fn seal(
        &self,
        random: &dyn RandomSource,
        key: &SecretKey,
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<EncryptedEnvelope>;

    fn open(
        &self,
        key: &SecretKey,
        envelope: &EncryptedEnvelope,
        aad: &[u8],
    ) -> Result<Vec<u8>>;
}

pub trait KeyDeriver {
    fn derive_key(
        &self,
        input_key: &SecretKey,
        salt: Option<&[u8]>,
        info: &[u8],
    ) -> Result<SecretKey>;
}

pub trait RandomSource {
    fn fill_bytes(&self, destination: &mut [u8]) -> Result<()>;
}
