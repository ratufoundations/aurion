use crate::error::ExecutionError;

pub const STORE_KEY_DOMAIN: &[u8] = b"AURION_STORE_KEY_NAMESPACE_V1";
pub const NAMESPACE_DIGEST_LEN: usize = 32;

/// Kunci partisi penyimpanan yang mengisolasi namespace setiap Keeper.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoreKey {
    namespace_digest: [u8; NAMESPACE_DIGEST_LEN],
    prefix: [u8; 4],
    label: String,
}

impl StoreKey {
    /// Membuat StoreKey baru dari label namespace teks.
    /// Menggunakan BLAKE3 dengan pemisahan domain dan panjang label untuk anti-collision mutlak.
    pub fn new(label: &str) -> Result<Self, ExecutionError> {
        if label.is_empty() || label.len() > 64 {
            return Err(ExecutionError::InvalidStoreKey {
                reason: "Panjang label namespace harus antara 1 dan 64 karakter",
            });
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(STORE_KEY_DOMAIN);
        hasher.update(&(label.len() as u64).to_be_bytes());
        hasher.update(label.as_bytes());
        let digest = *hasher.finalize().as_bytes();

        let mut prefix = [0u8; 4];
        prefix.copy_from_slice(&digest[0..4]);

        Ok(Self {
            namespace_digest: digest,
            prefix,
            label: label.to_string(),
        })
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn namespace_digest(&self) -> &[u8; 32] {
        &self.namespace_digest
    }

    #[must_use]
    pub fn prefix(&self) -> [u8; 4] {
        self.prefix
    }

    /// Mengkualifikasi kunci pengguna lokal menjadi kunci global berpartisi.
    /// Struktur: [namespace_digest 32-byte] || [user_key] (Fixed-Width Prefix-Free Guarantee).
    #[must_use]
    pub fn qualify(&self, user_key: &[u8]) -> Vec<u8> {
        let mut qualified = Vec::with_capacity(NAMESPACE_DIGEST_LEN + user_key.len());
        qualified.extend_from_slice(&self.namespace_digest);
        qualified.extend_from_slice(user_key);
        qualified
    }

    #[must_use]
    pub fn owns(&self, qualified_key: &[u8]) -> bool {
        qualified_key.starts_with(&self.namespace_digest)
    }
}

/// Jendela akses penyimpanan yang dibatasi hanya pada StoreKey pemilik.
#[derive(Debug)]
pub struct NamespaceStore<'a> {
    store_key: StoreKey,
    cache: &'a mut crate::cache::TransactionalCache,
}

impl<'a> NamespaceStore<'a> {
    pub fn new(store_key: StoreKey, cache: &'a mut crate::cache::TransactionalCache) -> Self {
        Self { store_key, cache }
    }

    pub fn get(&mut self, user_key: &[u8]) -> Result<Option<Vec<u8>>, ExecutionError> {
        let qualified = self.store_key.qualify(user_key);
        self.cache.get(&qualified)
    }

    pub fn set(&mut self, user_key: &[u8], value: &[u8]) -> Result<(), ExecutionError> {
        let qualified = self.store_key.qualify(user_key);
        self.cache.set(qualified, value.to_vec())
    }

    pub fn remove(&mut self, user_key: &[u8]) -> Result<(), ExecutionError> {
        let qualified = self.store_key.qualify(user_key);
        self.cache.remove(&qualified)
    }

    /// Menolak akses penulisan langsung jika kunci mentah tidak diawali oleh digest namespace sendiri.
    pub fn write_raw(&mut self, raw_key: &[u8], value: &[u8]) -> Result<(), ExecutionError> {
        if !self.store_key.owns(raw_key) {
            let mut attempted = [0u8; 4];
            if raw_key.len() >= 4 {
                attempted.copy_from_slice(&raw_key[0..4]);
            }
            return Err(ExecutionError::UnauthorizedStoreAccess {
                holder: self.store_key.label.clone(),
                attempted_prefix: attempted,
            });
        }
        self.cache.set(raw_key.to_vec(), value.to_vec())
    }
}
