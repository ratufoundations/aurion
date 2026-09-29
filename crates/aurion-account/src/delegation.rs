use crate::{address::AccountId, error::AccountError};
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};
use std::collections::BTreeMap;

/// Tag pemisahan domain kanonikal untuk otorisasi delegasi per-perangkat.
pub const DELEGATION_DOMAIN_TAG: &[u8] = b"AURION_DELEGATED_APPROVAL_V1";

/// Otorisasi delegasi yang ditandatangani device key.
///
/// Otorisasi mengikat `chain_id`, alamat akun, penandatangan, nonce akun, dan
/// digest aksi sehingga tidak dapat dipindah ke rantai, akun, atau aksi lain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedApproval {
    pub chain_id: u64,
    pub account: AccountId,
    pub signer: PublicKeyBytes,
    pub nonce: u64,
    pub action_digest: Hash256,
    pub signature: SignatureBytes,
}

impl DelegatedApproval {
    /// Komitmen biner otorisasi delegasi.
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(DELEGATION_DOMAIN_TAG);
        hasher.update(&self.chain_id.to_le_bytes());
        hasher.update(&self.account);
        hasher.update(&self.signer);
        hasher.update(&self.nonce.to_le_bytes());
        hasher.update(&self.action_digest);
        *hasher.finalize().as_bytes()
    }
}

/// Konteks eksekusi yang wajib diikat oleh otorisasi delegasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationContext {
    pub chain_id: u64,
    pub account: AccountId,
    pub signer: PublicKeyBytes,
    pub nonce: u64,
    pub action_digest: Hash256,
}

impl DelegationContext {
    #[must_use]
    pub const fn new(
        chain_id: u64,
        account: AccountId,
        signer: PublicKeyBytes,
        nonce: u64,
        action_digest: Hash256,
    ) -> Self {
        Self {
            chain_id,
            account,
            signer,
            nonce,
            action_digest,
        }
    }
}

/// Buku besar nonce aksi per akun: menolak replay otorisasi delegasi.
///
/// Seluruh validasi dijalankan sebelum mutasi apa pun, sehingga otorisasi yang
/// ditolak tidak pernah menaikkan nonce (sifat atomik).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DelegationLedger {
    consumed: BTreeMap<(AccountId, u64), Hash256>,
    next_nonce: BTreeMap<AccountId, u64>,
}

impl DelegationLedger {
    #[must_use]
    pub fn new() -> Self {
        Self {
            consumed: BTreeMap::new(),
            next_nonce: BTreeMap::new(),
        }
    }

    /// Nonce berikutnya yang diharapkan untuk akun (0 bila belum ada aksi).
    #[must_use]
    pub fn next_nonce(&self, account: &AccountId) -> u64 {
        self.next_nonce.get(account).copied().unwrap_or(0)
    }

    /// `true` bila kombinasi akun dan nonce sudah pernah dieksekusi.
    #[must_use]
    pub fn is_consumed(&self, account: &AccountId, nonce: u64) -> bool {
        self.consumed.contains_key(&(*account, nonce))
    }

    /// Eksekusi otorisasi delegasi setelah seluruh binding divalidasi.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::InvalidChainId` untuk chain salah,
    /// `AccountError::SignerMismatch` untuk akun/penandatangan salah,
    /// `AccountError::ActionDigestMismatch` untuk digest aksi berbeda,
    /// `AccountError::ReplayDetected` untuk nonce yang sudah dieksekusi,
    /// `AccountError::InvalidNonce` untuk nonce tidak berurutan, dan
    /// `AccountError::InvalidSignature` bila tanda tangan tidak sah.
    pub fn execute(
        &mut self,
        approval: &DelegatedApproval,
        context: &DelegationContext,
    ) -> Result<(), AccountError> {
        if approval.chain_id != context.chain_id {
            return Err(AccountError::InvalidChainId {
                expected: context.chain_id,
                got: approval.chain_id,
            });
        }
        if approval.account != context.account || approval.signer != context.signer {
            return Err(AccountError::SignerMismatch);
        }
        if approval.action_digest != context.action_digest {
            return Err(AccountError::ActionDigestMismatch);
        }
        if self.is_consumed(&context.account, context.nonce) {
            return Err(AccountError::ReplayDetected {
                account: context.account,
                nonce: context.nonce,
            });
        }
        let expected_nonce = self.next_nonce(&context.account);
        if approval.nonce != expected_nonce {
            return Err(AccountError::InvalidNonce {
                expected: expected_nonce,
                got: approval.nonce,
            });
        }
        SignatureVerifier::verify_single(&approval.signer, &approval.digest(), &approval.signature)
            .map_err(|_| AccountError::InvalidSignature)?;

        // Seluruh validasi lolos: mutasi state dilakukan secara atomik.
        let advanced = context
            .nonce
            .checked_add(1)
            .ok_or(AccountError::ArithmeticOverflow)?;
        self.consumed
            .insert((context.account, context.nonce), context.action_digest);
        self.next_nonce.insert(context.account, advanced);
        Ok(())
    }
}
