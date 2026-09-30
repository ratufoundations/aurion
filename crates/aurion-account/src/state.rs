use crate::{
    address::AccountId,
    error::AccountError,
    lifecycle::MIN_ACCOUNT_RESERVE_QUANTA,
    multisig::MultiSigPolicy,
    policy::SpendingPolicy,
    role::{DeviceRecord, DeviceRole, Role, RolePromotion, MIN_VALIDATOR_STAKE_QUANTA},
    rotation::KeyRotationProof,
};
use aurion_core::types::Quanta;
use aurion_criptografi::{Hash256, PublicKeyBytes};
use std::collections::BTreeMap;

pub const MAX_DEVICES_PER_ACCOUNT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SovereignAccount {
    pub account_id: Hash256,
    pub balance: Quanta,
    pub nonce: u64,
    pub devices: BTreeMap<PublicKeyBytes, DeviceRecord>,
    pub policy: SpendingPolicy,
    /// Peran protokol akun (`StandardUser` secara bawaan).
    pub role: Role,
    /// Stake yang disetor untuk peran validator (satuan Quanta).
    pub staked_quanta: Quanta,
    /// Kebijakan multi-sig otorisasi mutasi kebijakan akun.
    pub multisig: MultiSigPolicy,
}

impl SovereignAccount {
    /// Inisialisasi akun baru dengan satu Master Device (ala registrasi pertama WA)
    #[must_use]
    pub fn new(
        account_id: AccountId,
        master_device_key: PublicKeyBytes,
        current_time: u64,
    ) -> Self {
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
            role: Role::StandardUser,
            staked_quanta: 0,
            multisig: MultiSigPolicy::single(master_device_key),
        }
    }

    /// Registrasi akun baru dengan pendanaan awal minimum (pertahanan state bloat).
    ///
    /// # Errors
    /// Mengembalikan `AccountError::BelowDustThreshold` bila pendanaan awal
    /// berada di bawah `MIN_ACCOUNT_RESERVE_QUANTA`.
    pub fn register(
        account_id: AccountId,
        master_device_key: PublicKeyBytes,
        current_time: u64,
        initial_funding: Quanta,
    ) -> Result<Self, AccountError> {
        if initial_funding < MIN_ACCOUNT_RESERVE_QUANTA {
            return Err(AccountError::BelowDustThreshold {
                amount: initial_funding,
                minimum: MIN_ACCOUNT_RESERVE_QUANTA,
            });
        }
        let mut account = Self::new(account_id, master_device_key, current_time);
        account.balance = initial_funding;
        Ok(account)
    }

    /// Komitmen digest kanonikal state akun.
    ///
    /// Iterasi perangkat mengikuti urutan `BTreeMap` dan kebijakan multi-sig
    /// diringkas menjadi satu digest, sehingga hasilnya tidak bergantung pada
    /// urutan penautan perangkat.
    #[must_use]
    pub fn state_digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_SOVEREIGN_ACCOUNT_V1");
        hasher.update(&self.account_id);
        hasher.update(&self.balance.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        hasher.update(&[self.role.code()]);
        hasher.update(&self.staked_quanta.to_le_bytes());
        hasher.update(&self.multisig.digest());
        for (device_key, record) in &self.devices {
            hasher.update(device_key);
            hasher.update(&[record.role.code()]);
            hasher.update(&record.expires_at.to_le_bytes());
        }
        *hasher.finalize().as_bytes()
    }

    /// Pastikan penandatangan adalah perangkat Master yang terdaftar.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::DeviceNotRegistered` bila kunci tidak dikenal
    /// dan `AccountError::MasterPrivilegeRequired` bila perannya bukan Master.
    fn require_master(&self, signer_device: &PublicKeyBytes) -> Result<(), AccountError> {
        let record = self
            .devices
            .get(signer_device)
            .ok_or(AccountError::DeviceNotRegistered(*signer_device))?;
        if record.role != DeviceRole::Master {
            return Err(AccountError::MasterPrivilegeRequired);
        }
        Ok(())
    }

    /// Tautkan perangkat baru (ala Scan QR `WhatsApp`).
    ///
    /// # Errors
    /// Mengembalikan error bila signer tidak terdaftar/master atau batas perangkat tercapai.
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

    /// Cabut akses perangkat (ala Log out linked device di WA).
    ///
    /// # Errors
    /// Mengembalikan error bila signer tidak terdaftar/master atau master mencoba mencabut diri.
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

    /// Eksekusi pengeluaran dana yang diajukan oleh suatu perangkat.
    ///
    /// # Errors
    /// Mengembalikan error bila nonce, izin perangkat, kebijakan, atau saldo tidak valid.
    pub fn authorize_transfer(
        &mut self,
        signer_device: &PublicKeyBytes,
        amount: Quanta,
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
        self.balance =
            self.balance
                .checked_sub(amount)
                .ok_or(AccountError::InsufficientBalance {
                    available: self.balance,
                    required: amount,
                })?;
        self.nonce = self
            .nonce
            .checked_add(1)
            .ok_or(AccountError::ArithmeticOverflow)?;
        tracing::debug!(account = ?self.account_id, role = ?device.role, new_balance = self.balance, "Kebijakan akun diperbarui");
        Ok(())
    }

    /// Promosi `StandardUser` -> `ValidatorCandidate` dengan stake dan tanda tangan master.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::SignerMismatch` bila otorisasi tidak terikat pada
    /// akun ini, `AccountError::InvalidNonce` bila nonce tidak cocok,
    /// `AccountError::InvalidSignature` bila tanda tangan master tidak sah,
    /// `AccountError::MasterPrivilegeRequired` bila penandatangan bukan Master,
    /// `AccountError::UnauthorizedRole` bila transisi peran tidak sah, dan
    /// `AccountError::InsufficientStake` bila stake di bawah `MIN_VALIDATOR_STAKE_QUANTA`.
    pub fn promote_to_candidate(
        &mut self,
        promotion: &RolePromotion,
        master_key: &PublicKeyBytes,
    ) -> Result<(), AccountError> {
        self.apply_role_change(
            promotion,
            master_key,
            Role::StandardUser,
            Role::ValidatorCandidate,
        )
    }

    /// Aktivasi `ValidatorCandidate` -> `ActiveValidator`.
    ///
    /// # Errors
    /// Sama seperti [`SovereignAccount::promote_to_candidate`], namun transisi yang
    /// diterima adalah kandidat -> aktif dan stake wajib tetap memenuhi ambang.
    pub fn activate_validator(
        &mut self,
        promotion: &RolePromotion,
        master_key: &PublicKeyBytes,
    ) -> Result<(), AccountError> {
        self.apply_role_change(
            promotion,
            master_key,
            Role::ValidatorCandidate,
            Role::ActiveValidator,
        )
    }

    /// Terapkan transisi peran setelah seluruh validasi lolos (atomik).
    ///
    /// # Errors
    /// Mengembalikan error bertipe bila binding otorisasi, tanda tangan, otoritas
    /// master, transisi peran, atau ambang stake tidak sah.
    fn apply_role_change(
        &mut self,
        promotion: &RolePromotion,
        master_key: &PublicKeyBytes,
        from: Role,
        to: Role,
    ) -> Result<(), AccountError> {
        if promotion.account != self.account_id {
            return Err(AccountError::SignerMismatch);
        }
        if promotion.nonce != self.nonce {
            return Err(AccountError::InvalidNonce {
                expected: self.nonce,
                got: promotion.nonce,
            });
        }
        if promotion.new_role != to {
            return Err(AccountError::UnauthorizedRole {
                expected: to,
                actual: promotion.new_role,
            });
        }
        promotion.verify(master_key)?;
        self.require_master(master_key)?;
        if self.role != from {
            return Err(AccountError::UnauthorizedRole {
                expected: from,
                actual: self.role,
            });
        }
        if to.is_privileged() && promotion.stake_quanta < MIN_VALIDATOR_STAKE_QUANTA {
            return Err(AccountError::InsufficientStake {
                provided: promotion.stake_quanta,
                required: MIN_VALIDATOR_STAKE_QUANTA,
            });
        }
        let advanced = self
            .nonce
            .checked_add(1)
            .ok_or(AccountError::ArithmeticOverflow)?;
        // Seluruh validasi lolos: mutasi diterapkan secara atomik.
        self.role = to;
        self.staked_quanta = if to.is_privileged() {
            promotion.stake_quanta
        } else {
            0
        };
        self.nonce = advanced;
        Ok(())
    }

    /// Demosi ke `StandardUser` dan pencabutan seluruh hak istimewa seketika.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::DeviceNotRegistered`/`AccountError::MasterPrivilegeRequired`
    /// bila penandatangan bukan Master, dan `AccountError::UnauthorizedRole` bila
    /// akun sudah berperan `StandardUser`.
    pub fn demote_to_standard_user(
        &mut self,
        signer_device: &PublicKeyBytes,
    ) -> Result<(), AccountError> {
        self.require_master(signer_device)?;
        if self.role == Role::StandardUser {
            return Err(AccountError::UnauthorizedRole {
                expected: Role::ValidatorCandidate,
                actual: Role::StandardUser,
            });
        }
        self.role = Role::StandardUser;
        self.staked_quanta = 0;
        Ok(())
    }

    /// Rotasi kunci master: kunci lama dicabut, kunci baru didaftarkan sebagai Master.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::InvalidSignature`/`InvalidNonce`/`SignerMismatch`
    /// dari verifikasi bukti, `AccountError::DeviceNotRegistered` bila kunci lama tidak
    /// terdaftar, `AccountError::MasterPrivilegeRequired` bila kunci lama bukan Master,
    /// dan `AccountError::InvalidThreshold` bila kebijakan multi-sig tidak dapat
    /// menggantikan penandatangan lama.
    pub fn rotate_master_key(
        &mut self,
        proof: &KeyRotationProof,
        current_time: u64,
    ) -> Result<(), AccountError> {
        proof.verify(&self.account_id, self.nonce)?;
        let old_role = self
            .devices
            .get(&proof.old_key)
            .map(|record| record.role)
            .ok_or(AccountError::DeviceNotRegistered(proof.old_key))?;
        if old_role != DeviceRole::Master {
            return Err(AccountError::MasterPrivilegeRequired);
        }
        let rotated_policy = self
            .multisig
            .replace_signer(&proof.old_key, proof.new_key)?;
        let advanced = self
            .nonce
            .checked_add(1)
            .ok_or(AccountError::ArithmeticOverflow)?;
        // Seluruh validasi lolos: mutasi diterapkan secara atomik.
        self.devices.remove(&proof.old_key);
        self.devices.insert(
            proof.new_key,
            DeviceRecord {
                device_key: proof.new_key,
                role: DeviceRole::Master,
                registered_at: current_time,
                expires_at: 0,
            },
        );
        self.multisig = rotated_policy;
        self.nonce = advanced;
        Ok(())
    }

    /// Mutasi kebijakan multi-sig akun.
    ///
    /// Kebijakan baru ditolak bila akan mencabut seluruh otorisasi master aktif
    /// (state bricked) atau bila ambangnya tidak dapat dipenuhi.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::DeviceNotRegistered`/`AccountError::MasterPrivilegeRequired`
    /// bila penandatangan bukan Master, dan `AccountError::BrickedStateMutation`
    /// bila kebijakan baru tidak lagi memuat seluruh perangkat master aktif.
    pub fn update_multisig_policy(
        &mut self,
        signer_device: &PublicKeyBytes,
        new_policy: MultiSigPolicy,
    ) -> Result<(), AccountError> {
        self.require_master(signer_device)?;
        let orphaned = self
            .devices
            .iter()
            .any(|(key, record)| record.role == DeviceRole::Master && !new_policy.is_signer(key));
        if orphaned {
            return Err(AccountError::BrickedStateMutation);
        }
        self.multisig = new_policy;
        Ok(())
    }
}
