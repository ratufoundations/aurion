use crate::error::GenesisError;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;

/// Rasio presisi kanonikal: 1 AUR = 10^10 Quanta (definisi pusat dari `aurion-core`).
pub use aurion_core::types::QUANTA_PER_AUR;

/// Pasokan siklus sintetis Aurion: 66 juta AUR per siklus pencetakan.
pub const CYCLE_SUPPLY_AUR: u128 = 66_000_000;

/// Pasokan siklus dalam Quanta: 66M AUR × 10^10 = `660_000_000_000_000_000`.
pub const TOTAL_CYCLE_SUPPLY_QUANTA: u128 = 660_000_000_000_000_000;

/// Alokasi rasio basis-poin untuk akun Creator: 21%.
pub const CREATOR_ALLOCATION_BPS: u64 = 2_100;

/// Alokasi rasio basis-poin untuk Reservoir: 79%.
pub const RESERVOIR_ALLOCATION_BPS: u64 = 7_900;

/// Alokasi akun Creator dalam Quanta: 21% × 660 × 10^15.
pub const CREATOR_ALLOCATION_QUANTA: u128 = 138_600_000_000_000_000;

/// Alokasi Reservoir dalam Quanta: 79% × 660 × 10^15.
pub const RESERVOIR_ALLOCATION_QUANTA: u128 = 521_400_000_000_000_000;

/// Serialisasi `u128` sebagai string desimal untuk mencegah pemotongan
/// presisi IEEE-754 pada konsumen JSON (presisi aman maksimal 2^53).
///
/// # Errors
/// Mengembalikan galat serializer bila representasi string gagal ditulis.
pub fn serialize_u128_as_string<S>(val: &u128, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.collect_str(val)
}

/// Deserialisasi `u128` dari representasi string desimal ATAU integer JSON.
///
/// Menolak representasi pecahan/float dan angka negatif agar presisi moneter
/// u128 utuh dan zero-float tetap terjamin.
///
/// # Errors
/// Mengembalikan galat deserialisasi untuk string non-desimal, angka negatif,
/// atau representasi float.
pub fn deserialize_u128_from_string_or_int<'de, D>(d: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    struct U128StringOrInt;

    impl de::Visitor<'_> for U128StringOrInt {
        type Value = u128;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("bukan string desimal atau integer tak-bertanda (u128)")
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
            Ok(u128::from(v))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            u128::try_from(v)
                .map_err(|_| de::Error::custom("nilai negatif dilarang untuk satuan Quanta"))
        }

        fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E> {
            Ok(v)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            v.parse::<u128>()
                .map_err(|_| de::Error::custom("string bukan representasi desimal u128 yang valid"))
        }
    }

    d.deserialize_any(U128StringOrInt)
}

/// Peran alokasi leksikografis deterministik pada state root genesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationRole {
    Creator,
    Reservoir,
}

impl AllocationRole {
    /// Kode biner kanonikal stabil untuk komitmen state root.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Creator => 0,
            Self::Reservoir => 1,
        }
    }
}

/// Alokasi saldo awal satu akun pada genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisAllocation {
    pub account_id: [u8; 32],
    pub role: AllocationRole,
    #[serde(
        serialize_with = "serialize_u128_as_string",
        deserialize_with = "deserialize_u128_from_string_or_int"
    )]
    pub amount_quanta: u128,
}

/// Validator perdana pada genesis (stake dinominalkan dalam Quanta).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisValidator {
    pub consensus_pubkey: [u8; 32],
    #[serde(
        serialize_with = "serialize_u128_as_string",
        deserialize_with = "deserialize_u128_from_string_or_int"
    )]
    pub stake_quanta: u128,
}

/// Parameter konsensus awal yang dikunci di blok genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsensusGenesisParams {
    pub epoch_duration_blocks: u64,
    pub active_set_capacity: u32,
}

/// Konfigurasi lengkap genesis (skema JSON yang divalidasi ketat).
///
/// Jumlah seluruh alokasi + stake validator WAJIB konservatif terhadap
/// `TOTAL_CYCLE_SUPPLY_QUANTA` (lihat `validate_supply_conservation`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisConfig {
    pub chain_id: u64,
    pub genesis_time: u64,
    pub consensus: ConsensusGenesisParams,
    pub allocations: Vec<GenesisAllocation>,
    pub initial_validators: Vec<GenesisValidator>,
}

impl GenesisConfig {
    /// Validasi konservasi moneter: total alokasi akun + total stake validator
    /// harus tepat sama dengan `TOTAL_CYCLE_SUPPLY_QUANTA`.
    ///
    /// # Errors
    /// Mengembalikan `SchemaViolation` bila terjadi luapan penjumlahan dan
    /// `SupplyConservationMismatch` bila total tidak konservatif.
    pub fn validate_supply_conservation(&self) -> Result<(), GenesisError> {
        let mut sum: u128 = 0;
        for alloc in &self.allocations {
            sum = sum
                .checked_add(alloc.amount_quanta)
                .ok_or_else(|| GenesisError::SchemaViolation("Overflow total alokasi".into()))?;
        }
        for val in &self.initial_validators {
            sum = sum.checked_add(val.stake_quanta).ok_or_else(|| {
                GenesisError::SchemaViolation("Overflow total stake validator".into())
            })?;
        }
        if sum != TOTAL_CYCLE_SUPPLY_QUANTA {
            return Err(GenesisError::SupplyConservationMismatch {
                calculated: sum,
                expected: TOTAL_CYCLE_SUPPLY_QUANTA,
            });
        }
        Ok(())
    }

    /// Validasi himpunan validator perdana: tidak boleh kosong dan tidak boleh
    /// mengandung `consensus_pubkey` duplikat.
    ///
    /// # Errors
    /// Mengembalikan `ZeroValidators` bila tidak ada validator dan
    /// `DuplicateValidatorPubkey` bila terdapat duplikasi pubkey.
    pub fn validate_validator_set(&self) -> Result<(), GenesisError> {
        if self.initial_validators.is_empty() {
            return Err(GenesisError::ZeroValidators);
        }
        let mut seen = HashSet::with_capacity(self.initial_validators.len());
        for val in &self.initial_validators {
            if !seen.insert(val.consensus_pubkey) {
                return Err(GenesisError::DuplicateValidatorPubkey(val.consensus_pubkey));
            }
        }
        Ok(())
    }

    /// Menjalankan seluruh validasi skema genesis secara berurutan:
    /// himpunan validator, lalu konservasi moneter.
    ///
    /// # Errors
    /// Mengembalikan galat kategori skema pertama yang dilanggar.
    pub fn validate(&self) -> Result<(), GenesisError> {
        self.validate_validator_set()?;
        self.validate_supply_conservation()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn base_config() -> GenesisConfig {
        GenesisConfig {
            chain_id: 1001,
            genesis_time: 1_700_000_000,
            consensus: ConsensusGenesisParams {
                epoch_duration_blocks: 2_850,
                active_set_capacity: 21,
            },
            allocations: vec![
                GenesisAllocation {
                    account_id: [1u8; 32],
                    role: AllocationRole::Creator,
                    amount_quanta: CREATOR_ALLOCATION_QUANTA,
                },
                GenesisAllocation {
                    account_id: [2u8; 32],
                    role: AllocationRole::Reservoir,
                    amount_quanta: RESERVOIR_ALLOCATION_QUANTA,
                },
            ],
            initial_validators: vec![GenesisValidator {
                consensus_pubkey: [3u8; 32],
                stake_quanta: 0,
            }],
        }
    }

    #[test]
    fn split_bps_is_exact() {
        let creator = CREATOR_ALLOCATION_QUANTA;
        let reservoir = RESERVOIR_ALLOCATION_QUANTA;
        assert_eq!(creator + reservoir, TOTAL_CYCLE_SUPPLY_QUANTA);
        assert_eq!(
            creator * 10_000,
            TOTAL_CYCLE_SUPPLY_QUANTA * u128::from(CREATOR_ALLOCATION_BPS)
        );
        assert_eq!(
            reservoir * 10_000,
            TOTAL_CYCLE_SUPPLY_QUANTA * u128::from(RESERVOIR_ALLOCATION_BPS)
        );
    }

    #[test]
    fn u128_string_round_trip() {
        let cfg = base_config();
        let json = serde_json::to_string(&cfg).expect("serialisasi berhasil");
        assert!(json.contains("\"amount_quanta\":\"138600000000000000\""));
        let back: GenesisConfig =
            serde_json::from_str(&json).expect("deserialisasi dari string berhasil");
        assert_eq!(back, cfg);
    }

    #[test]
    fn validate_supply_accepts_conservative_config() {
        base_config().validate().expect("konfigurasi valid");
    }

    #[test]
    fn validate_rejects_non_conservative_supply() {
        let mut cfg = base_config();
        cfg.allocations[0].amount_quanta = CREATOR_ALLOCATION_QUANTA - 1;
        match cfg.validate() {
            Err(GenesisError::SupplyConservationMismatch {
                calculated,
                expected,
            }) => {
                assert_eq!(calculated + 1, expected);
            }
            other => panic!("Ekspektasi SupplyConservationMismatch, dapat: {other:?}"),
        }
    }
}
