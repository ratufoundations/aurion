#![forbid(unsafe_code)]

pub mod delegation;
pub mod domain;
pub mod error;
pub mod wallet;

pub use delegation::DeviceCertificate;
pub use domain::Domain;
pub use error::WalletError;
pub use wallet::{AurionWallet, LinkedDeviceSession};
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DOMAIN_TEST_ATTACKER;
    use aurion_core::State;
    use aurion_criptografi::Keypair;
    #[test]
    fn test_whatsapp_device_pairing_and_delegation_lifecycle() {
        let root_identity = Keypair::generate();
        let wallet = AurionWallet::new(root_identity, "aurion.finance.consortium");
        let phone_device = Keypair::generate();
        let phone_pubkey = phone_device.public_key_bytes();
        let current_time = 1_700_000_000u64;
        let validity_seconds = 86400 * 30;
        let cert = wallet.delegate_device(phone_pubkey, current_time, validity_seconds);
        assert!(cert.verify(current_time + 100, &wallet.domain_tag).is_ok());
        let wrong_domain = Domain::custom(DOMAIN_TEST_ATTACKER);
        assert_eq!(
            cert.verify(current_time + 100, &wrong_domain).unwrap_err(),
            WalletError::DomainMismatch
        );
        let expired_time = current_time + (86400 * 31);
        assert_eq!(
            cert.verify(expired_time, &wallet.domain_tag).unwrap_err(),
            WalletError::DelegationExpired {
                expired_at: current_time + validity_seconds,
                current_time: expired_time
            }
        );
    }
    #[test]
    fn test_wallet_tx_generation_and_state_verification() {
        let alice_identity = Keypair::generate();
        let bob_identity = Keypair::generate();
        let alice_wallet = AurionWallet::new(alice_identity, "aurion.mainnet");
        let bob_pubkey = bob_identity.public_key_bytes();
        let mut state = State::new();
        state.insert_account(
            alice_wallet.public_key(),
            aurion_core::Account::new(500_000, 0),
        );
        let tx = alice_wallet.build_transaction(bob_pubkey, 120_000, 0);
        state
            .apply_transaction(&tx)
            .expect("Transaksi dari wallet harus sah");
        let alice_acc = state
            .get_account(&alice_wallet.public_key())
            .expect("test operation should succeed");
        let bob_acc = state
            .get_account(&bob_pubkey)
            .expect("test operation should succeed");
        assert_eq!(alice_acc.balance, 380_000);
        assert_eq!(alice_acc.nonce, 1);
        assert_eq!(bob_acc.balance, 120_000);
    }
}
