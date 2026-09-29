use crate::error::ValidatorError;

/// Status siklus hidup validator (mesin keadaan V0–V3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidatorStatus {
    /// Lulus pendaftaran, sedang menjalani masa percobaan (*probation window*).
    Probation,
    /// Lulus masa percobaan; satu-satunya status yang boleh dipilih ke `ActiveSet`.
    Eligible,
    /// Terpilih pada batas epoch; ikut dihitung dalam kuorum konsensus.
    ActiveSet,
    /// Ditahan otomatis karena melewati ambang blok terlewat berturut-turut.
    Jailed,
    /// Ditangguhkan karena penalti berat (slashing/putusan dewan/tata kelola).
    Suspended,
    /// Keluar permanen (terminal); tidak ada transisi keluar.
    Retired,
}

/// Seluruh status siklus hidup; dipakai untuk audit kelengkapan matriks transisi.
pub const ALL_VALIDATOR_STATUSES: [ValidatorStatus; 6] = [
    ValidatorStatus::Probation,
    ValidatorStatus::Eligible,
    ValidatorStatus::ActiveSet,
    ValidatorStatus::Jailed,
    ValidatorStatus::Suspended,
    ValidatorStatus::Retired,
];

/// Alasan penangguhan (*suspension*) validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SuspensionReason {
    /// Pemotongan stake berat pada atau di atas ambang `SEVERE_SLASH_RATE_BPS`.
    SevereSlashing,
    /// Putusan dewan pengawas (`GuardCouncil`).
    GuardCouncilVerdict,
    /// Keputusan tata kelola protokol.
    GovernanceVote,
}

impl SuspensionReason {
    /// Kode biner kanonikal untuk komitmen digest (stabil lintas versi).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::SevereSlashing => 0,
            Self::GuardCouncilVerdict => 1,
            Self::GovernanceVote => 2,
        }
    }
}

impl ValidatorStatus {
    /// Kode biner kanonikal untuk komitmen digest (stabil lintas versi).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Probation => 0,
            Self::Eligible => 1,
            Self::ActiveSet => 2,
            Self::Jailed => 3,
            Self::Suspended => 4,
            Self::Retired => 5,
        }
    }

    /// `true` bila status ini masuk perhitungan seleksi `ActiveSet` pada batas
    /// epoch: kandidat `Eligible` untuk promosi maupun anggota `ActiveSet` yang
    /// mempertahankan kursinya. Status `Probation`, `Jailed`, `Suspended`, dan
    /// `Retired` tidak pernah terpilih sehingga himpunan aktif tidak pernah
    /// kosong akibat rotasi beruntun.
    #[must_use]
    pub const fn is_selectable(self) -> bool {
        matches!(self, Self::Eligible | Self::ActiveSet)
    }

    /// `true` bila status ini dihitung dalam kuorum konsensus aktif.
    #[must_use]
    pub const fn counts_toward_quorum(self) -> bool {
        matches!(self, Self::ActiveSet)
    }

    /// `true` bila status bersifat terminal (tanpa transisi keluar).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Retired)
    }

    /// `true` bila validator sedang menjalani masa percobaan.
    #[must_use]
    pub const fn is_probation(self) -> bool {
        matches!(self, Self::Probation)
    }

    /// `true` bila validator sedang ditahan atau ditangguhkan.
    #[must_use]
    pub const fn is_restricted(self) -> bool {
        matches!(self, Self::Jailed | Self::Suspended)
    }

    /// Matriks transisi sah (V1/V3).
    ///
    /// `Probation` tidak pernah boleh melompat langsung ke `ActiveSet`, dan
    /// `Jailed`/`Suspended` wajib kembali melalui `Eligible` sebelum dapat
    /// dipilih ulang pada batas epoch.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Probation | Self::Suspended,
                Self::Eligible | Self::Retired
            ) | (
                Self::Eligible,
                Self::ActiveSet | Self::Suspended | Self::Retired
            ) | (
                Self::ActiveSet,
                Self::Eligible | Self::Jailed | Self::Suspended | Self::Retired
            ) | (
                Self::Jailed,
                Self::Eligible | Self::Suspended | Self::Retired
            )
        )
    }

    /// Penegakan transisi: menolak perubahan status di luar matriks sah.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::InvalidStatusTransition` berisi status asal
    /// dan tujuan yang ditolak.
    pub fn authorize_transition(self, next: Self) -> Result<(), ValidatorError> {
        if self.can_transition_to(next) {
            Ok(())
        } else {
            Err(ValidatorError::InvalidStatusTransition {
                from: self,
                to: next,
            })
        }
    }
}
