use dragonforge_crypto::{OsRandom, RandomSource};

use crate::{Result, VaultError};

const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.?";

#[derive(Debug, Clone, Copy)]
pub struct PasswordPolicy {
    pub length: usize,
    pub lowercase: bool,
    pub uppercase: bool,
    pub digits: bool,
    pub symbols: bool,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            length: 24,
            lowercase: true,
            uppercase: true,
            digits: true,
            symbols: true,
        }
    }
}

pub fn generate_password(policy: PasswordPolicy) -> Result<String> {
    if policy.length < 8 {
        return Err(VaultError::InvalidPasswordPolicy(
            "password length must be at least 8",
        ));
    }

    let mut required = Vec::new();
    let mut alphabet = Vec::new();
    for (enabled, class) in [
        (policy.lowercase, LOWER),
        (policy.uppercase, UPPER),
        (policy.digits, DIGITS),
        (policy.symbols, SYMBOLS),
    ] {
        if enabled {
            required.push(class);
            alphabet.extend_from_slice(class);
        }
    }

    if alphabet.is_empty() {
        return Err(VaultError::InvalidPasswordPolicy(
            "at least one character class must be enabled",
        ));
    }
    if required.len() > policy.length {
        return Err(VaultError::InvalidPasswordPolicy(
            "password is too short for required character classes",
        ));
    }

    let random = OsRandom;
    let mut out = Vec::with_capacity(policy.length);

    for class in required {
        out.push(random_choice(&random, class)?);
    }
    while out.len() < policy.length {
        out.push(random_choice(&random, &alphabet)?);
    }

    secure_shuffle(&random, &mut out)?;
    String::from_utf8(out).map_err(|_| VaultError::InvalidPasswordPolicy("invalid UTF-8"))
}

fn random_choice(random: &dyn RandomSource, choices: &[u8]) -> Result<u8> {
    let zone = u8::MAX - (u8::MAX % choices.len() as u8);
    loop {
        let mut byte = [0_u8; 1];
        random.fill_bytes(&mut byte)?;
        if byte[0] < zone {
            return Ok(choices[usize::from(byte[0]) % choices.len()]);
        }
    }
}

fn secure_shuffle(random: &dyn RandomSource, bytes: &mut [u8]) -> Result<()> {
    for index in (1..bytes.len()).rev() {
        let upper = index + 1;
        let zone = u32::MAX - (u32::MAX % upper as u32);
        let selected = loop {
            let mut raw = [0_u8; 4];
            random.fill_bytes(&mut raw)?;
            let value = u32::from_le_bytes(raw);
            if value < zone {
                break (value as usize) % upper;
            }
        };
        bytes.swap(index, selected);
    }
    Ok(())
}
