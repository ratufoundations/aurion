use crate::{
    error::AccountError,
    policy::SpendingPolicy,
    role::{DeviceRecord, DeviceRole},
};
use aurion_criptografi::{Hash256, PublicKeyBytes};
use std::collections::BTreeMap;

pub const MAX_DEVICES_PER_ACCOUNT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SovereignAccount {
    pub account_id: Hash256,
    pub balance: u64,
    pub nonce: u64,
    pub devices: BTreeMap<PublicKeyBytes, DeviceRecord>,
    pub policy: SpendingPolicy,
}

impl SovereignAccount {
    /// Inisialisasi akun baru dengan satu Master Device (ala registrasi pertama WA)
    pub fn new(account_id: Hash256, master_device_key: PublicKeyBytes, current_time: u64) -> Self {
        let mut devices = BTreeMap::new();
        devices.insert(
            master_device_key,
            DeviceRecord {
                device_key: master_device_key,
                role: DeviceRole::Master,
                registered_at: current_time,
                expires_at: 0,
            },
        );
        Self {
            account_id,
            balance: 0,
            nonce: 0,
            devices,
            policy: SpendingPolicy::default(),
        }
    }

    /// Tautkan perangkat baru (ala Scan QR WhatsApp)
    pub fn link_device(
        &mut self,
        signer_device: &PublicKeyBytes,
        new_device: PublicKeyBytes,
        role: DeviceRole,
        current_time: u64,
        validity_duration_seconds: u64,
    ) -> Result<(), AccountError> {
        let signer = self
            .devices
            .get(signer_device)
            .ok_or(AccountError::DeviceNotRegistered(*signer_device))?;
        if signer.role != DeviceRole::Master {
            return Err(AccountError::MasterPrivilegeRequired);
        }
        if self.devices.len() >= MAX_DEVICES_PER_ACCOUNT {
            return Err(AccountError::DeviceLimitReached(MAX_DEVICES_PER_ACCOUNT));
        }
        let expires_at = if role == DeviceRole::Master {
            0
        } else {
            current_time.saturating_add(validity_duration_seconds)
        };
        self.devices.insert(
            new_device,
            DeviceRecord {
                device_key: new_device,
                role,
                registered_at: current_time,
                expires_at,
            },
        );
        Ok(())
    }

    /// Cabut akses perangkat (ala Log out linked device di WA)
    pub fn revoke_device(
        &mut self,
        signer_device: &PublicKeyBytes,
        target_device: &PublicKeyBytes,
    ) -> Result<(), AccountError> {
        let signer = self
            .devices
            .get(signer_device)
            .ok_or(AccountError::DeviceNotRegistered(*signer_device))?;
        if signer.role != DeviceRole::Master {
            return Err(AccountError::MasterPrivilegeRequired);
        }
        if let Some(target) = self.devices.get(target_device) {
            if target.role == DeviceRole::Master && signer_device == target_device {
                return Err(AccountError::CannotRemoveMasterDevice);
            }
        }
        self.devices.remove(target_device);
        Ok(())
    }

    /// Eksekusi pengeluaran dana yang diajukan oleh suatu perangkat
    pub fn authorize_transfer(
        &mut self,
        signer_device: &PublicKeyBytes,
        amount: u64,
        nonce: u64,
        current_time: u64,
    ) -> Result<(), AccountError> {
        if nonce != self.nonce {
            return Err(AccountError::InvalidNonce {
                expected: self.nonce,
                got: nonce,
            });
        }
        let device = self
            .devices
            .get(signer_device)
            .ok_or(AccountError::DeviceNotRegistered(*signer_device))?;
        if !device.is_valid(current_time) {
            return Err(AccountError::DeviceExpired(*signer_device));
        }
        match device.role {
            DeviceRole::Master => {}
            DeviceRole::DailyOperator => {
                self.policy.check_and_update(amount, current_time)?;
            }
            DeviceRole::ReadOnly => {
                return Err(AccountError::MasterPrivilegeRequired);
            }
        }
        if self.balance < amount {
            tracing::warn!(account = ?self.account_id, available = self.balance, required = amount, "Transfer ditolak: saldo tidak mencukupi");
            return Err(AccountError::InsufficientBalance {
                available: self.balance,
                required: amount,
            });
        }
        self.balance -= amount;
        self.nonce += 1;
        tracing::debug!(account = ?self.account_id, role = ?device.role, new_balance = self.balance, "Kebijakan akun diperbarui");
        Ok(())
    }
}
