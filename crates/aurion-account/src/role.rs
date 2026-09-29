use crate::{address::AccountId, error::AccountError};
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};

/// Stake minimum (satuan Quanta `u64`) untuk mengaktifkan peran validator.
pub const MIN_VALIDATOR_STAKE_QUANTA: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceRole {
    /// Pemilik penuh: bisa mutasi saldo tanpa batas, menambah & mencopot perangkat lain
    Master,
    /// Perangkat harian (Laptop/Tablet): tunduk pada limit transaksi & kuota harian
    DailyOperator,
    /// Perangkat pantau: hanya boleh membaca saldo / menerima pesan tanpa izin transfer
    ReadOnly,
}

impl DeviceRole {
    /// Kode biner kanonikal untuk komitmen digest (stabil lintas versi).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Master => 0,
            Self::DailyOperator => 1,
            Self::ReadOnly => 2,
        }
    }
}

/// Peran protokol tingkat akun (RBAC berdaulat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Pengguna biasa: tanpa hak istimewa protokol.
    StandardUser,
    /// Kandidat validator: sudah menyetor stake, belum boleh memilih.
    ValidatorCandidate,
    /// Validator aktif: boleh mengirim suara dan proposal konsensus.
    ActiveValidator,
    /// Anggota `GuardCouncil`: hanya peran ini yang boleh memicu slashing.
    GuardCouncil,
}

/// Aksi istimewa protokol yang wajib melewati pemeriksaan peran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleAction {
    Vote,
    Propose,
    Slash,
}

impl RoleAction {
    /// Peran minimum yang berwenang menjalankan aksi ini.
    #[must_use]
    pub const fn required_role(self) -> Role {
        match self {
            Self::Vote | Self::Propose => Role::ActiveValidator,
            Self::Slash => Role::GuardCouncil,
        }
    }
}

impl Role {
    /// Kode biner kanonikal untuk komitmen digest (stabil lintas versi).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::StandardUser => 0,
            Self::ValidatorCandidate => 1,
            Self::ActiveValidator => 2,
            Self::GuardCouncil => 3,
        }
    }

    /// `true` bila peran ini menahan hak istimewa protokol.
    #[must_use]
    pub const fn is_privileged(self) -> bool {
        !matches!(self, Self::StandardUser)
    }

    #[must_use]
    pub const fn can_vote(self) -> bool {
        matches!(self, Self::ActiveValidator)
    }

    #[must_use]
    pub const fn can_propose(self) -> bool {
        matches!(self, Self::ActiveValidator)
    }

    #[must_use]
    pub const fn can_slash(self) -> bool {
        matches!(self, Self::GuardCouncil)
    }

    /// `true` bila peran ini berwenang atas aksi yang diminta.
    #[must_use]
    pub const fn can(self, action: RoleAction) -> bool {
        match action {
            RoleAction::Vote => self.can_vote(),
            RoleAction::Propose => self.can_propose(),
            RoleAction::Slash => self.can_slash(),
        }
    }

    /// Penegakan kapabilitas: menolak aksi istimewa tanpa peran yang sah.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::UnauthorizedRole` berisi peran yang dibutuhkan
    /// dan peran yang dimiliki.
    pub fn authorize(self, action: RoleAction) -> Result<(), AccountError> {
        if self.can(action) {
            Ok(())
        } else {
            Err(AccountError::UnauthorizedRole {
                expected: action.required_role(),
                actual: self,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRecord {
    pub device_key: PublicKeyBytes,
    pub role: DeviceRole,
    pub registered_at: u64,
    pub expires_at: u64,
}

impl DeviceRecord {
    #[must_use]
    pub fn is_valid(&self, current_time: u64) -> bool {
        if self.expires_at == 0 {
            true
        } else {
            current_time <= self.expires_at
        }
    }
}

/// Otorisasi promosi peran yang ditandatangani kunci master akun.
///
/// Otorisasi mengikat alamat akun, peran tujuan, jumlah stake, dan nonce akun
/// sehingga tidak dapat dipakai ulang untuk akun atau transisi peran lain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolePromotion {
    pub account: AccountId,
    pub new_role: Role,
    pub stake_quanta: u64,
    pub nonce: u64,
    pub signature: SignatureBytes,
}

impl RolePromotion {
    /// Tag pemisahan domain kanonikal untuk otorisasi promosi peran.
    pub const DOMAIN_TAG: &'static [u8] = b"AURION_ROLE_PROMOTION_V1";

    /// Komitmen biner otorisasi: `DOMAIN_TAG || account || role || stake || nonce`.
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(Self::DOMAIN_TAG);
        hasher.update(&self.account);
        hasher.update(&[self.new_role.code()]);
        hasher.update(&self.stake_quanta.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Verifikasi tanda tangan otorisasi terhadap kunci master yang diharapkan.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::InvalidSignature` bila tanda tangan tidak cocok.
    pub fn verify(&self, master_key: &PublicKeyBytes) -> Result<(), AccountError> {
        SignatureVerifier::verify_single(master_key, &self.digest(), &self.signature)
            .map_err(|_| AccountError::InvalidSignature)
    }
}
