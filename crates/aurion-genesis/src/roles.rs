use aurion_criptografi::PublicKeyBytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationTopology {
    /// Kunci publik simpul pembuat blok dan penandatangan konsensus BFT
    pub validators: BTreeSet<PublicKeyBytes>,
    /// Kunci publik simpul penjaga/pengawas (autoritas verifikasi & guard faucet)
    pub guards: BTreeSet<PublicKeyBytes>,
}

impl FederationTopology {
    pub fn new(
        validators: impl IntoIterator<Item = PublicKeyBytes>,
        guards: impl IntoIterator<Item = PublicKeyBytes>,
    ) -> Self {
        Self {
            validators: validators.into_iter().collect(),
            guards: guards.into_iter().collect(),
        }
    }

    pub fn is_validator(&self, key: &PublicKeyBytes) -> bool {
        self.validators.contains(key)
    }

    pub fn is_guard(&self, key: &PublicKeyBytes) -> bool {
        self.guards.contains(key)
    }
}
