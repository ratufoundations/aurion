use crate::error::AccountError;
use aurion_criptografi::{Hash256, PublicKeyBytes};

/// Tag pemisahan domain kanonikal untuk derivasi alamat berdaulat.
pub const ADDRESS_DOMAIN_TAG: &[u8] = b"AURION_ADDR_CANONICAL_V1";

/// Ukuran biner alamat akun berdaulat (bita).
pub const ACCOUNT_ID_LEN: usize = 32;

/// Alamat akun berdaulat: digest 32 bita hasil derivasi ber-domain.
pub type AccountId = Hash256;

/// Turunkan alamat akun berdaulat dari public key `Ed25519`.
///
/// Derivasi bersifat deterministik dan satu arah (BLAKE3 atas
/// `ADDRESS_DOMAIN_TAG || public_key`), sehingga kunci yang sama selalu
/// menghasilkan alamat yang sama pada setiap simpul.
#[must_use]
pub fn derive_account_id(public_key: &PublicKeyBytes) -> AccountId {
    derive_account_id_with_domain(ADDRESS_DOMAIN_TAG, public_key)
}

/// Turunkan alamat dengan tag domain eksplisit.
///
/// Fungsi ini dipakai untuk audit pemisahan domain: mengganti tag wajib
/// menghasilkan alamat yang berbeda untuk kunci yang sama.
#[must_use]
pub fn derive_account_id_with_domain(domain_tag: &[u8], public_key: &PublicKeyBytes) -> AccountId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain_tag);
    hasher.update(public_key);
    *hasher.finalize().as_bytes()
}

/// Parsing alamat baku dari potongan bita (wire/ledger) tanpa panic.
///
/// # Errors
/// Mengembalikan `AccountError::InvalidAccountIdLength` bila panjang bukan 32 bita.
pub fn parse_account_id(bytes: &[u8]) -> Result<AccountId, AccountError> {
    <[u8; ACCOUNT_ID_LEN]>::try_from(bytes)
        .map_err(|_| AccountError::InvalidAccountIdLength { got: bytes.len() })
}
