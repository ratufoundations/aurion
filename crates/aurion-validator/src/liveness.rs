use crate::error::ValidatorError;
use aurion_account::AccountId;
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes, SignatureBytes, SignatureVerifier};

/// Ambang blok terlewat berturut-turut sebelum validator ditahan otomatis.
pub const MAX_MISSED_BLOCKS_THRESHOLD: u64 = 10;

/// Masa denda blok minimum sebelum permohonan `unjail` boleh diproses.
pub const UNJAIL_COOLDOWN_BLOCKS: u64 = 1_200;

/// Masa denda tambahan untuk validator yang ditangguhkan karena pelanggaran berat.
pub const SUSPENSION_COOLDOWN_BLOCKS: u64 = 4_800;

/// Ambang pemotongan berat (BPS) yang memicu penangguhan otomatis.
pub const SEVERE_SLASH_RATE_BPS: u64 = 1_000;

/// Hasil pelaporan satu blok terhadap rekaman validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReport {
    /// Blok selama probation tercatat; jendela berjalan menuju kelulusan.
    ProbationProgress {
        /// Blok tercatat pada jendela berjalan.
        observed: u64,
        /// Blok yang dibutuhkan untuk lulus.
        required: u64,
    },
    /// Pelanggaran liveness selama probation: jendela dimulai ulang.
    ProbationWindowReset {
        /// Panjang rentetan kegagalan berurutan.
        miss_streak: u64,
    },
    /// Validator lulus probation otomatis pada pelaporan blok ini.
    Graduated {
        /// Blok tempat kelulusan terjadi.
        height: u64,
    },
    /// Blok aktif berhasil ditandatangani.
    Signed {
        /// Rentetan blok terlewat setelah direset ke nol.
        consecutive_missed_blocks: u64,
    },
    /// Blok aktif terlewat, belum melewati ambang penahanan.
    MissRecorded {
        /// Rentetan blok terlewat berurutan.
        consecutive_missed_blocks: u64,
    },
    /// Ambang blok terlewat terlampaui: validator ditahan otomatis.
    AutoJailed {
        /// Rentetan blok terlewat yang memicu penahanan.
        consecutive_missed_blocks: u64,
        /// Blok tempat penahanan dijatuhkan.
        height: u64,
    },
}

/// Permohonan pemulihan status yang ditandatangani kunci master akun.
///
/// Otorisasi mengikat alamat akun, blok permohonan, dan nonce `unjail` akun
/// sehingga tanda tangan yang sama tidak dapat dipakai ulang (*anti-replay*).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnjailRequest {
    /// Akun berdaulat validator yang memohon pemulihan.
    pub account: AccountId,
    /// Kunci master akun yang menandatangani permohonan.
    pub master_key: PublicKeyBytes,
    /// Blok saat permohonan diajukan.
    pub requested_at_block: u64,
    /// Nonce `unjail` akun (wajib sama dengan rekaman validator).
    pub nonce: u64,
    /// Tanda tangan `Ed25519` atas [`UnjailRequest::digest`].
    pub signature: SignatureBytes,
}

impl UnjailRequest {
    /// Tag pemisahan domain kanonikal untuk otorisasi pemulihan.
    pub const DOMAIN_TAG: &'static [u8] = b"AURION_VALIDATOR_UNJAIL_V1";

    /// Komitmen biner yang ditandatangani kunci master akun.
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(Self::DOMAIN_TAG);
        hasher.update(&self.account);
        hasher.update(&self.master_key);
        hasher.update(&self.requested_at_block.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Terbitkan permohonan yang ditandatangani kunci master akun.
    #[must_use]
    pub fn issue(
        account: AccountId,
        master_key: &Keypair,
        requested_at_block: u64,
        nonce: u64,
    ) -> Self {
        let mut request = Self {
            account,
            master_key: master_key.public_key_bytes(),
            requested_at_block,
            nonce,
            signature: [0_u8; 64],
        };
        request.signature = master_key.sign(&request.digest());
        request
    }

    /// Verifikasi binding akun, nonce, dan keaslian tanda tangan master.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::InvalidUnjailAuthorization` bila akun tidak
    /// cocok, nonce tidak sesuai (indikasi replay), atau tanda tangan tidak sah.
    pub fn verify(
        &self,
        expected_account: &AccountId,
        expected_nonce: u64,
    ) -> Result<(), ValidatorError> {
        if self.account != *expected_account || self.nonce != expected_nonce {
            return Err(ValidatorError::InvalidUnjailAuthorization);
        }
        SignatureVerifier::verify_single(&self.master_key, &self.digest(), &self.signature)
            .map_err(|_| ValidatorError::InvalidUnjailAuthorization)
    }
}
