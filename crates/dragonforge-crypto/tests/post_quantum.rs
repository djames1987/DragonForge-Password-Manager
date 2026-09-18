use dragonforge_crypto::{
    CryptoError, HybridKemCiphertext, HybridKemKeyPair, MlDsa65KeyPair, MlDsa65VerifyingKey,
    MlKem768KeyPair, MlKem768PublicKey, ML_DSA_65_PUBLIC_KEY_LEN, ML_DSA_65_SIGNATURE_LEN,
    ML_KEM_768_CIPHERTEXT_LEN, ML_KEM_768_PUBLIC_KEY_LEN, hybrid_kem_encapsulate,
    ml_kem_768_encapsulate,
};

#[test]
fn ml_kem_768_round_trip() {
    let recipient = MlKem768KeyPair::generate();
    let (ciphertext, sender_key) = ml_kem_768_encapsulate(recipient.public_key()).unwrap();

    assert_eq!(ciphertext.len(), ML_KEM_768_CIPHERTEXT_LEN);

    let recipient_key = recipient.decapsulate(&ciphertext).unwrap();
    assert!(sender_key.ct_eq(&recipient_key));
}

#[test]
fn ml_kem_768_seed_reconstruction_preserves_public_key() {
    let original = MlKem768KeyPair::generate();
    let seed = original.export_seed();
    let restored = MlKem768KeyPair::from_seed(&seed).unwrap();

    assert_eq!(original.public_key(), restored.public_key());
}

#[test]
fn ml_kem_768_public_key_rejects_wrong_length() {
    assert_eq!(
        MlKem768PublicKey::from_bytes(&vec![0_u8; ML_KEM_768_PUBLIC_KEY_LEN - 1]),
        Err(CryptoError::InvalidMlKemPublicKey)
    );
}

#[test]
fn ml_kem_768_decapsulation_rejects_wrong_ciphertext_length() {
    let recipient = MlKem768KeyPair::generate();
    assert!(matches!(
        recipient.decapsulate(&vec![0_u8; ML_KEM_768_CIPHERTEXT_LEN - 1]),
        Err(CryptoError::InvalidMlKemCiphertext)
    ));
}

#[test]
fn ml_dsa_65_sign_verify_round_trip() {
    let keypair = MlDsa65KeyPair::generate();
    let message = b"DragonForge device authorization";
    let signature = keypair.sign(message);

    assert_eq!(signature.len(), ML_DSA_65_SIGNATURE_LEN);
    keypair.verifying_key().verify(message, &signature).unwrap();
}

#[test]
fn ml_dsa_65_rejects_modified_message() {
    let keypair = MlDsa65KeyPair::generate();
    let signature = keypair.sign(b"approved message");

    assert_eq!(
        keypair
            .verifying_key()
            .verify(b"modified message", &signature),
        Err(CryptoError::SignatureVerificationFailed)
    );
}

#[test]
fn ml_dsa_65_rejects_modified_signature() {
    let keypair = MlDsa65KeyPair::generate();
    let message = b"device enrollment";
    let mut signature = keypair.sign(message);
    signature[0] ^= 0x80;

    assert!(keypair.verifying_key().verify(message, &signature).is_err());
}

#[test]
fn ml_dsa_65_seed_reconstruction_preserves_verifying_key() {
    let original = MlDsa65KeyPair::generate();
    let seed = original.export_seed();
    let restored = MlDsa65KeyPair::from_seed(&seed).unwrap();

    assert_eq!(original.verifying_key(), restored.verifying_key());

    let signature = restored.sign(b"restored signer");
    original
        .verifying_key()
        .verify(b"restored signer", &signature)
        .unwrap();
}

#[test]
fn ml_dsa_65_verifying_key_rejects_wrong_length() {
    assert_eq!(
        MlDsa65VerifyingKey::from_bytes(&vec![0_u8; ML_DSA_65_PUBLIC_KEY_LEN - 1]),
        Err(CryptoError::InvalidMlDsaVerifyingKey)
    );
}

#[test]
fn hybrid_x25519_mlkem768_round_trip() {
    let recipient = HybridKemKeyPair::generate();
    let context = b"dragonforge/device-enrollment/v1";

    let (ciphertext, sender_key) =
        hybrid_kem_encapsulate(recipient.public_key(), context).unwrap();
    let recipient_key = recipient.decapsulate(&ciphertext, context).unwrap();

    assert!(sender_key.ct_eq(&recipient_key));
}

#[test]
fn hybrid_context_is_cryptographically_bound() {
    let recipient = HybridKemKeyPair::generate();
    let (ciphertext, sender_key) =
        hybrid_kem_encapsulate(recipient.public_key(), b"context-a").unwrap();

    let recipient_key = recipient.decapsulate(&ciphertext, b"context-b").unwrap();
    assert!(!sender_key.ct_eq(&recipient_key));
}

#[test]
fn hybrid_rejects_empty_context() {
    let recipient = HybridKemKeyPair::generate();

    assert!(matches!(
        hybrid_kem_encapsulate(recipient.public_key(), b""),
        Err(CryptoError::EmptyContext)
    ));
}

#[test]
fn hybrid_rejects_non_contributory_x25519_peer() {
    let recipient = HybridKemKeyPair::generate();
    let bad_public = dragonforge_crypto::HybridKemPublicKey {
        x25519: [0_u8; 32],
        ml_kem_768: recipient.public_key().ml_kem_768.clone(),
    };

    assert!(matches!(
        hybrid_kem_encapsulate(&bad_public, b"test"),
        Err(CryptoError::NonContributoryX25519)
    ));
}

#[test]
fn hybrid_ciphertext_serializes_round_trip() {
    let recipient = HybridKemKeyPair::generate();
    let (ciphertext, sender_key) =
        hybrid_kem_encapsulate(recipient.public_key(), b"serialization-test").unwrap();

    let json = serde_json::to_vec(&ciphertext).unwrap();
    let decoded: HybridKemCiphertext = serde_json::from_slice(&json).unwrap();
    let recipient_key = recipient
        .decapsulate(&decoded, b"serialization-test")
        .unwrap();

    assert!(sender_key.ct_eq(&recipient_key));
}

#[test]
fn private_key_debug_output_is_redacted() {
    let kem = MlKem768KeyPair::generate();
    let dsa = MlDsa65KeyPair::generate();
    let hybrid = HybridKemKeyPair::generate();

    assert!(format!("{kem:?}").contains("[REDACTED]"));
    assert!(format!("{dsa:?}").contains("[REDACTED]"));
    assert!(format!("{hybrid:?}").contains("[REDACTED]"));
}
