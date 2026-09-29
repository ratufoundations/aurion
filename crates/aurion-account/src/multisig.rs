use crate::error::AccountError;
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};
use std::collections::BTreeSet;

/// Satu persetujuan multi-sig atas digest aksi tertentu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSigApproval {
    pub signer: PublicKeyBytes,
    pub signature: SignatureBytes,
}

impl MultiSigApproval {
    /// Buat persetujuan dari penandatangan dan tanda tangan mentahnya.
    #[must_use]
    pub const fn new(signer: PublicKeyBytes, signature: SignatureBytes) -> Self {
        Self { signer, signature }
    }
}

/// Kebijakan multi-sig akun: daftar penandatangan unik dan ambang `M`.
///
/// Konstruksi hanya sah bila `M >= 1`, `M <= N`, dan seluruh penandatangan
/// unik. Pelanggaran mengembalikan `AccountError::InvalidThreshold` sehingga
/// akun tidak pernah berada pada konfigurasi yang mustahil dipenuhi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSigPolicy {
    signers: BTreeSet<PublicKeyBytes>,
    threshold: usize,
}

impl MultiSigPolicy {
    /// Bangun kebijakan multi-sig tervalidasi.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::InvalidThreshold` bila `M == 0`, `M > N`,
    /// atau daftar penandatangan memuat duplikat.
    pub fn new(signers: &[PublicKeyBytes], threshold: usize) -> Result<Self, AccountError> {
        let mut unique = BTreeSet::new();
        let mut has_duplicate = false;
        for signer in signers {
            if !unique.insert(*signer) {
                has_duplicate = true;
            }
        }
        if has_duplicate || threshold == 0 || threshold > unique.len() {
            return Err(AccountError::InvalidThreshold {
                approvals: unique.len(),
                threshold,
                signers: signers.len(),
            });
        }
        Ok(Self {
            signers: unique,
            threshold,
        })
    }

    /// Kebijakan penandatangan tunggal (master akun) yang selalu valid.
    #[must_use]
    pub fn single(signer: PublicKeyBytes) -> Self {
        let mut signers = BTreeSet::new();
        signers.insert(signer);
        Self {
            signers,
            threshold: 1,
        }
    }

    /// Ganti satu penandatangan tanpa mengubah ambang, memakai validasi kanonikal.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::SignerMismatch` bila `old` bukan penandatangan,
    /// atau `AccountError::InvalidThreshold` bila hasil akhir melanggar `M <= N`.
    pub fn replace_signer(
        &self,
        old: &PublicKeyBytes,
        new: PublicKeyBytes,
    ) -> Result<Self, AccountError> {
        if !self.signers.contains(old) {
            return Err(AccountError::SignerMismatch);
        }
        let mut signers = self.signers.clone();
        signers.remove(old);
        signers.insert(new);
        let canonical: Vec<PublicKeyBytes> = signers.iter().copied().collect();
        Self::new(&canonical, self.threshold)
    }

    #[must_use]
    pub const fn threshold(&self) -> usize {
        self.threshold
    }

    #[must_use]
    pub fn signer_count(&self) -> usize {
        self.signers.len()
    }

    #[must_use]
    pub fn signers(&self) -> &BTreeSet<PublicKeyBytes> {
        &self.signers
    }

    #[must_use]
    pub fn is_signer(&self, candidate: &PublicKeyBytes) -> bool {
        self.signers.contains(candidate)
    }

    /// `true` bila jumlah persetujuan memenuhi ambang `M`.
    #[must_use]
    pub const fn has_quorum(&self, approvals: usize) -> bool {
        approvals >= self.threshold
    }

    /// Komitmen kanonikal kebijakan: daftar penandatangan terurut + ambang `M`.
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_MULTISIG_POLICY_V1");
        for signer in &self.signers {
            hasher.update(signer);
        }
        hasher.update(
            &u64::try_from(self.threshold)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        *hasher.finalize().as_bytes()
    }

    /// Verifikasi kumpulan persetujuan atas satu digest aksi.
    ///
    /// Penandatangan non-anggota ditolak (`SignerMismatch`), persetujuan ganda
    /// dari kunci yang sama ditolak (`InvalidThreshold`), dan jumlah persetujuan
    /// sah yang kurang dari `M` ditolak (`InvalidThreshold`).
    ///
    /// # Errors
    /// Mengembalikan error bertipe bila penandatangan, tanda tangan, atau ambang tidak sah.
    pub fn verify_approvals(
        &self,
        digest: &Hash256,
        approvals: &[MultiSigApproval],
    ) -> Result<usize, AccountError> {
        let mut seen = BTreeSet::new();
        let mut verified: usize = 0;
        for approval in approvals {
            if !self.is_signer(&approval.signer) {
                return Err(AccountError::SignerMismatch);
            }
            if !seen.insert(approval.signer) {
                return Err(AccountError::InvalidThreshold {
                    approvals: seen.len(),
                    threshold: self.threshold,
                    signers: self.signers.len(),
                });
            }
            if SignatureVerifier::verify_single(&approval.signer, digest, &approval.signature)
                .is_err()
            {
                return Err(AccountError::InvalidSignature);
            }
            verified = verified.saturating_add(1);
        }
        if !self.has_quorum(verified) {
            return Err(AccountError::InvalidThreshold {
                approvals: verified,
                threshold: self.threshold,
                signers: self.signers.len(),
            });
        }
        Ok(verified)
    }
}
