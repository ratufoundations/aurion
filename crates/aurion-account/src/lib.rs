#![forbid(unsafe_code)]

pub mod error;
pub mod policy;
pub mod role;
pub mod state;

pub use error::AccountError;
pub use policy::SpendingPolicy;
pub use role::{DeviceRecord, DeviceRole};
pub use state::{SovereignAccount, MAX_DEVICES_PER_ACCOUNT};

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_criptografi::Keypair;
    #[test]
    fn test_whatsapp_style_multidevice_account_lifecycle() {
        let master_phone = Keypair::generate();
        let laptop = Keypair::generate();
        let tablet = Keypair::generate();
        let master_pk = master_phone.public_key_bytes();
        let laptop_pk = laptop.public_key_bytes();
        let tablet_pk = tablet.public_key_bytes();
        let current_time = 1_700_000_000u64;
        let account_id = [0x55; 32];
        let mut account = SovereignAccount::new(account_id, master_pk, current_time);
        account.balance = 500_000;
        account
            .link_device(
                &master_pk,
                laptop_pk,
                DeviceRole::DailyOperator,
                current_time,
                86400 * 30,
            )
            .unwrap();
        account
            .authorize_transfer(&laptop_pk, 30_000, 0, current_time + 10)
            .unwrap();
        assert_eq!(account.balance, 470_000);
        assert_eq!(account.nonce, 1);
        let err = account
            .authorize_transfer(&laptop_pk, 60_000, 1, current_time + 20)
            .unwrap_err();
        assert_eq!(
            err,
            AccountError::ExceedsPerTxLimit {
                amount: 60_000,
                limit: 50_000
            }
        );
        account
            .authorize_transfer(&master_pk, 100_000, 1, current_time + 30)
            .unwrap();
        assert_eq!(account.balance, 370_000);
        assert_eq!(account.nonce, 2);
        let link_err = account
            .link_device(
                &laptop_pk,
                tablet_pk,
                DeviceRole::ReadOnly,
                current_time,
                86400,
            )
            .unwrap_err();
        assert_eq!(link_err, AccountError::MasterPrivilegeRequired);
        account.revoke_device(&master_pk, &laptop_pk).unwrap();
        let revoked_err = account
            .authorize_transfer(&laptop_pk, 10_000, 2, current_time + 40)
            .unwrap_err();
        assert_eq!(revoked_err, AccountError::DeviceNotRegistered(laptop_pk));
    }
}
