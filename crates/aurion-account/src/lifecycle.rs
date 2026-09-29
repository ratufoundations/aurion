use crate::{error::AccountError, role::Role, state::SovereignAccount};

/// Reserve minimum (satuan Quanta `u64`) agar akun baru boleh terdaftar.
///
/// Nilai ini adalah ambang *placement* anti state-bloat, bukan kebijakan moneter.
pub const MIN_ACCOUNT_RESERVE_QUANTA: u64 = 1_000;

/// Hasil keputusan penerimaan transfer terhadap siklus hidup akun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOutcome {
    /// Penerima sudah ada; transfer berapa pun besarnya sah.
    ExistingAccount,
    /// Penerima baru dibuat dan wajib menahan reserve minimum.
    NewAccountReserved,
}

/// Kebijakan siklus hidup akun: pertahanan dust dan state bloat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountLifecycle;

impl AccountLifecycle {
    /// Terima atau tolak transfer berdasarkan keberadaan akun penerima.
    ///
    /// Transfer ke akun yang belum ada wajib menahan reserve minimum sehingga
    /// penyerang tidak dapat membuat akun tak terbatas dengan biaya nol.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::BelowDustThreshold` bila jumlah transfer ke
    /// akun baru berada di bawah `MIN_ACCOUNT_RESERVE_QUANTA` (termasuk nol).
    pub fn admit_transfer(
        target_exists: bool,
        amount: u64,
    ) -> Result<AdmissionOutcome, AccountError> {
        if !target_exists && amount < MIN_ACCOUNT_RESERVE_QUANTA {
            return Err(AccountError::BelowDustThreshold {
                amount,
                minimum: MIN_ACCOUNT_RESERVE_QUANTA,
            });
        }
        if target_exists {
            Ok(AdmissionOutcome::ExistingAccount)
        } else {
            Ok(AdmissionOutcome::NewAccountReserved)
        }
    }

    /// `true` bila akun adalah kandidat pruning: saldo nol, nonce sudah terpakai,
    /// tanpa stake, dan tanpa hak istimewa protokol.
    ///
    /// Representasi kanonikal akun nir-saldo adalah saldo `0` tanpa residu
    /// pecahan, sehingga sisa pembulatan tidak pernah menahan entri state.
    #[must_use]
    pub fn is_prunable(account: &SovereignAccount) -> bool {
        account.balance == 0
            && account.nonce > 0
            && account.staked_quanta == 0
            && account.role == Role::StandardUser
    }
}
