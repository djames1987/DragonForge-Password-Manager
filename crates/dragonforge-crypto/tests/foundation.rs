use dragonforge_crypto::{
    AeadCipher, Aes256GcmCipher, Argon2idConfig, Argon2idKdf, CipherSuite, CryptoError,
    EncryptedEnvelope, HkdfSha512, KeyDeriver, MIN_MEMORY_KIB, OsRandom, PasswordKdf, SecretKey,
    constant_time_eq, generate_secret_key, unwrap_key, wrap_key,
};

fn test_kdf() -> Argon2idKdf {
    Argon2idKdf::new(
        Argon2idConfig::new(MIN_MEMORY_KIB, 2, 1).expect("valid test KDF parameters"),
    )
    .expect("valid KDF")
}

#[test]
fn argon2id_is_deterministic_for_same_inputs() {
    let kdf = test_kdf();
    let salt = b"0123456789abcdef";
    let first = kdf.derive_key(b"correct horse battery staple", salt).unwrap();
    let second = kdf.derive_key(b"correct horse battery staple", salt).unwrap();
    assert!(first.ct_eq(&second));
}

#[test]
fn argon2id_changes_with_salt() {
    let kdf = test_kdf();
    let first = kdf.derive_key(b"same password", b"0123456789abcdef").unwrap();
    let second = kdf.derive_key(b"same password", b"fedcba9876543210").unwrap();
    assert!(!first.ct_eq(&second));
}

#[test]
fn argon2id_rejects_short_salt_and_weak_parameters() {
    let kdf = test_kdf();
    assert!(matches!(
        kdf.derive_key(b"password", b"too-short"),
        Err(CryptoError::InvalidSaltLength { .. })
    ));
    assert_eq!(
        Argon2idConfig::new(MIN_MEMORY_KIB - 1, 2, 1),
        Err(CryptoError::InvalidKdfParameters)
    );
}

#[test]
fn aes_gcm_round_trip_binds_aad() {
    let random = OsRandom;
    let key = generate_secret_key(&random).unwrap();
    let cipher = Aes256GcmCipher;
    let aad = b"vault=abc;item=42;schema=1";
    let envelope = cipher.seal(&random, &key, b"super secret password", aad).unwrap();

    assert_eq!(envelope.suite(), CipherSuite::Aes256GcmV1);
    assert_eq!(cipher.open(&key, &envelope, aad).unwrap(), b"super secret password");
    assert_eq!(
        cipher.open(&key, &envelope, b"vault=abc;item=43;schema=1"),
        Err(CryptoError::DecryptionFailed)
    );
}

#[test]
fn aes_gcm_rejects_ciphertext_tampering() {
    let random = OsRandom;
    let key = generate_secret_key(&random).unwrap();
    let cipher = Aes256GcmCipher;
    let envelope = cipher.seal(&random, &key, b"secret", b"aad").unwrap();
    let mut tampered_ciphertext = envelope.ciphertext().to_vec();
    tampered_ciphertext[0] ^= 0x80;
    let tampered = EncryptedEnvelope::from_parts(
        envelope.version(),
        envelope.suite(),
        envelope.nonce(),
        tampered_ciphertext,
    );

    assert_eq!(
        cipher.open(&key, &tampered, b"aad"),
        Err(CryptoError::DecryptionFailed)
    );
}

#[test]
fn aes_gcm_rejects_unsupported_envelope_version() {
    let random = OsRandom;
    let key = generate_secret_key(&random).unwrap();
    let cipher = Aes256GcmCipher;
    let envelope = cipher.seal(&random, &key, b"secret", b"aad").unwrap();
    let unsupported = EncryptedEnvelope::from_parts(
        99,
        envelope.suite(),
        envelope.nonce(),
        envelope.ciphertext().to_vec(),
    );

    assert_eq!(
        cipher.open(&key, &unsupported, b"aad"),
        Err(CryptoError::UnsupportedEnvelopeVersion(99))
    );
}

#[test]
fn hkdf_domain_separation_produces_distinct_keys() {
    let root = SecretKey::from_bytes([0x42; 32]);
    let deriver = HkdfSha512;
    let items = deriver.derive_key(&root, None, b"dragonforge/vault/items/v1").unwrap();
    let files = deriver.derive_key(&root, None, b"dragonforge/vault/files/v1").unwrap();
    assert!(!items.ct_eq(&files));
    assert_eq!(
        deriver.derive_key(&root, None, b""),
        Err(CryptoError::EmptyContext)
    );
}

#[test]
fn key_wrap_round_trip_is_context_bound() {
    let random = OsRandom;
    let wrapping_key = generate_secret_key(&random).unwrap();
    let child_key = generate_secret_key(&random).unwrap();
    let cipher = Aes256GcmCipher;
    let context = b"dragonforge/vault-master-key/v1";

    let envelope = wrap_key(&cipher, &random, &wrapping_key, &child_key, context).unwrap();
    let recovered = unwrap_key(&cipher, &wrapping_key, &envelope, context).unwrap();
    assert!(child_key.ct_eq(&recovered));
    assert!(unwrap_key(&cipher, &wrapping_key, &envelope, b"wrong-context").is_err());
}

#[test]
fn encrypted_envelope_serializes_and_round_trips() {
    let random = OsRandom;
    let key = generate_secret_key(&random).unwrap();
    let cipher = Aes256GcmCipher;
    let envelope = cipher.seal(&random, &key, b"payload", b"aad").unwrap();

    let encoded = serde_json::to_vec(&envelope).unwrap();
    let decoded: EncryptedEnvelope = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, envelope);
    assert_eq!(cipher.open(&key, &decoded, b"aad").unwrap(), b"payload");
}

#[test]
fn constant_time_comparison_handles_equal_and_unequal_inputs() {
    assert!(constant_time_eq(b"same", b"same"));
    assert!(!constant_time_eq(b"same", b"diff"));
    assert!(!constant_time_eq(b"short", b"longer"));
}

#[test]
fn secret_debug_output_is_redacted() {
    let key = SecretKey::from_bytes([0xAA; 32]);
    assert_eq!(format!("{key:?}"), "SecretKey([REDACTED])");
}
