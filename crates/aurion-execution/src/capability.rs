use crate::error::ExecutionError;
use crate::store_key::StoreKey;
use std::collections::BTreeMap;

pub const CAPABILITY_TOKEN_DOMAIN: &[u8] = b"AURION_CAPABILITY_TOKEN_V1";

/// Identitas modul yang diturunkan dari StoreKey partisi miliknya.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId([u8; 32]);

impl ModuleId {
    #[must_use]
    pub fn from_store_key(key: &StoreKey) -> Self {
        Self(*key.namespace_digest())
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Deskriptor kapabilitas izin akses.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Capability {
    name: String,
    digest: [u8; 32],
}

impl Capability {
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_CAPABILITY_NAME_V1");
        hasher.update(name.as_bytes());
        Self {
            name: name.to_string(),
            digest: *hasher.finalize().as_bytes(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

/// Bukti pemberian kapabilitas yang dicatat dalam registri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrant {
    pub capability: Capability,
    pub grantee: ModuleId,
    pub expiry_block: u64, // 0 = tanpa batas waktu
}

/// Handle otorisasi yang dibawa pemanggil untuk mengakses metode berhak istimewa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityHandle {
    capability: Capability,
    grantee: ModuleId,
    auth_token: [u8; 32],
}

impl CapabilityHandle {
    #[must_use]
    pub fn forged_for_test(capability: Capability, grantee: ModuleId) -> Self {
        Self {
            capability,
            grantee,
            auth_token: [0xFF; 32], // Token palsu yang pasti ditolak
        }
    }

    #[must_use]
    pub fn capability(&self) -> &Capability {
        &self.capability
    }

    #[must_use]
    pub fn grantee(&self) -> &ModuleId {
        &self.grantee
    }
}

/// Registri kapabilitas terpusat untuk otorisasi antar-Keeper (RFC-001).
#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    salt: [u8; 32],
    grants: BTreeMap<(Capability, ModuleId), CapabilityGrant>,
}

impl CapabilityRegistry {
    #[must_use]
    pub fn new(salt: [u8; 32]) -> Self {
        Self {
            salt,
            grants: BTreeMap::new(),
        }
    }

    pub fn grant(
        &mut self,
        capability: Capability,
        grantee: ModuleId,
        expiry_block: u64,
    ) -> Result<(), ExecutionError> {
        let grant = CapabilityGrant {
            capability: capability.clone(),
            grantee,
            expiry_block,
        };
        self.grants.insert((capability, grantee), grant);
        Ok(())
    }

    pub fn revoke(
        &mut self,
        capability: &Capability,
        grantee: &ModuleId,
    ) -> Result<(), ExecutionError> {
        self.grants.remove(&(capability.clone(), *grantee));
        Ok(())
    }

    pub fn issue_handle(
        &self,
        capability: &Capability,
        grantee: &ModuleId,
    ) -> Result<CapabilityHandle, ExecutionError> {
        if !self.grants.contains_key(&(capability.clone(), *grantee)) {
            return Err(ExecutionError::CapabilityMissing {
                capability: capability.name().to_string(),
                grantee: *grantee,
            });
        }

        let token = self.compute_token(capability, grantee);
        Ok(CapabilityHandle {
            capability: capability.clone(),
            grantee: *grantee,
            auth_token: token,
        })
    }

    pub fn authorize(
        &self,
        handle: &CapabilityHandle,
        current_block: u64,
    ) -> Result<(), ExecutionError> {
        let grant = self
            .grants
            .get(&(handle.capability.clone(), handle.grantee))
            .ok_or_else(|| ExecutionError::CapabilityMissing {
                capability: handle.capability.name().to_string(),
                grantee: handle.grantee,
            })?;

        if grant.expiry_block > 0 && current_block > grant.expiry_block {
            return Err(ExecutionError::CapabilityExpired {
                capability: handle.capability.name().to_string(),
                grantee: handle.grantee,
                expired_at: grant.expiry_block,
                current_block,
            });
        }

        let expected_token = self.compute_token(&handle.capability, &handle.grantee);
        if handle.auth_token != expected_token {
            return Err(ExecutionError::CapabilityMissing {
                capability: handle.capability.name().to_string(),
                grantee: handle.grantee,
            });
        }

        Ok(())
    }

    fn compute_token(&self, capability: &Capability, grantee: &ModuleId) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CAPABILITY_TOKEN_DOMAIN);
        hasher.update(&self.salt);
        hasher.update(capability.digest());
        hasher.update(grantee.as_bytes());
        *hasher.finalize().as_bytes()
    }
}
