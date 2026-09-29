#![forbid(unsafe_code)]

use aurion_criptografi::{CryptoError, Hasher, Keypair, SignatureVerifier};

#[test]
fn c0_hash_is_deterministic_across_payload_sizes() {
    for length in [0, 32, 1_024, 65_536] {
        let payload: Vec<u8> = (0_u8..=250).cycle().take(length).collect();
        let expected = Hasher::digest(&payload);

        for _ in 0..1_000 {
            assert_eq!(
                Hasher::digest(&payload),
                expected,
                "payload length {length}"
            );
        }
        assert_eq!(Hasher::digest_parallel(&payload), expected);
    }
}

#[test]
fn c1_hash_has_avalanche_behavior_for_single_bit_changes() {
    let original = [0_u8; 64];
    let original_hash = Hasher::digest(&original);
    let mut changed_output_bits = 0_u64;

    for byte_index in 0..original.len() {
        for bit_index in 0_u32..8 {
            let mut modified = original;
            modified[byte_index] ^= 1_u8 << bit_index;

            let modified_hash = Hasher::digest(&modified);
            changed_output_bits += original_hash
                .iter()
                .zip(modified_hash.iter())
                .map(|(left, right)| u64::from((left ^ right).count_ones()))
                .sum::<u64>();
        }
    }

    let compared_output_bits = 64_u64 * 8 * 256;
    assert!(
        changed_output_bits * 100 >= compared_output_bits * 45,
        "avalanche ratio fell below 45%"
    );
    assert!(
        changed_output_bits * 100 <= compared_output_bits * 55,
        "avalanche ratio exceeded 55%"
    );
}

#[test]
fn c2_ed25519_signature_round_trips() {
    let keypair = Keypair::from_bytes(&[0x42; 32]);
    let message = b"aurion cryptography module contract";
    let signature = keypair.sign(message);

    assert!(matches!(
        SignatureVerifier::verify_single(&keypair.public_key_bytes(), message, &signature),
        Ok(())
    ));
}

#[test]
fn c3_signature_rejects_a_tampered_message() {
    let keypair = Keypair::from_bytes(&[0x43; 32]);
    let message = b"canonical transaction payload";
    let signature = keypair.sign(message);
    let mut tampered_message = message.to_vec();
    tampered_message[0] ^= 1;

    assert!(matches!(
        SignatureVerifier::verify_single(
            &keypair.public_key_bytes(),
            &tampered_message,
            &signature
        ),
        Err(CryptoError::VerificationFailed)
    ));
}

#[test]
fn c4_signature_rejects_the_wrong_signer() {
    let signer = Keypair::from_bytes(&[0x44; 32]);
    let other_signer = Keypair::from_bytes(&[0x45; 32]);
    let message = b"signed only by the first key";
    let signature = signer.sign(message);

    assert!(matches!(
        SignatureVerifier::verify_single(&other_signer.public_key_bytes(), message, &signature),
        Err(CryptoError::VerificationFailed)
    ));
}

#[test]
fn c5_signature_mutation_and_noncanonical_scalar_are_rejected() {
    let keypair = Keypair::from_bytes(&[0x46; 32]);
    let message = b"signature canonicality";
    let valid_signature = keypair.sign(message);

    let mut mutated_signature = valid_signature;
    mutated_signature[0] ^= 1;
    assert!(matches!(
        SignatureVerifier::verify_single(&keypair.public_key_bytes(), message, &mutated_signature),
        Err(CryptoError::VerificationFailed)
    ));

    // Ed25519 subgroup order L in little-endian form. S == L is non-canonical;
    // canonical signatures require S < L.
    let scalar_order = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10,
    ];
    let mut noncanonical_signature = valid_signature;
    noncanonical_signature[32..].copy_from_slice(&scalar_order);

    assert!(matches!(
        SignatureVerifier::verify_single(
            &keypair.public_key_bytes(),
            message,
            &noncanonical_signature
        ),
        Err(CryptoError::VerificationFailed)
    ));
}

#[test]
fn c6_malformed_fixed_size_encodings_return_errors_without_panicking() {
    let message = b"malformed input safety";
    let invalid_public_key = [0xff; 32];
    let signature = [0; 64];
    assert!(matches!(
        SignatureVerifier::verify_single(&invalid_public_key, message, &signature),
        Err(CryptoError::InvalidPublicKey | CryptoError::VerificationFailed)
    ));

    let keypair = Keypair::from_bytes(&[0x47; 32]);
    for malformed_signature in [[0; 64], [0xff; 64]] {
        assert!(matches!(
            SignatureVerifier::verify_single(
                &keypair.public_key_bytes(),
                message,
                &malformed_signature
            ),
            Err(CryptoError::VerificationFailed | CryptoError::InvalidSignature)
        ));
    }
}

#[test]
fn c7_seed_derivation_is_deterministic_and_debug_redacts_private_key() {
    let seed = [0x48; 32];
    let first = Keypair::from_bytes(&seed);
    let second = Keypair::from_bytes(&seed);
    let message = b"deterministic key derivation";

    assert_eq!(first.private_key_bytes(), second.private_key_bytes());
    assert_eq!(first.public_key_bytes(), second.public_key_bytes());
    assert_eq!(first.sign(message), second.sign(message));

    let debug_output = format!("{first:?}");
    assert!(debug_output.contains("signing_key: \"[REDACTED]\""));
}
