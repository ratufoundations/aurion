use crate::{
    endorsement::{EndorsementLedger, MAX_ACTIVE_ENDORSEMENTS, MIN_ENDORSER_TENURE_BLOCKS},
    epoch::{
        select_active_set, ActiveSetSelection, EpochSchedule, EPOCH_LENGTH_BLOCKS,
        MAX_ACTIVE_VALIDATORS,
    },
    error::ValidatorError,
    liveness::{
        BlockReport, UnjailRequest, MAX_MISSED_BLOCKS_THRESHOLD, SEVERE_SLASH_RATE_BPS,
        SUSPENSION_COOLDOWN_BLOCKS, UNJAIL_COOLDOWN_BLOCKS,
    },
    proof::ProofOfPossession,
    record::ValidatorRecord,
    scoring::{accumulate_weight, effective_stake_quanta, slash_amount, BPS_DENOMINATOR},
    status::{SuspensionReason, ValidatorStatus},
};
use aurion_account::{AccountId, DeviceRole, Role, SovereignAccount, MIN_VALIDATOR_STAKE_QUANTA};
use aurion_criptografi::{Hash256, PublicKeyBytes};
use std::collections::{BTreeMap, BTreeSet};

/// Panjang jendela probation bawaan (1 minggu @2 detik/blok), selaras
/// `Agents.md` §5 dan `probation::DEFAULT_PROBATION_BLOCKS`.
pub const DEFAULT_PROBATION_WINDOW_BLOCKS: u64 = crate::probation::DEFAULT_PROBATION_BLOCKS;

/// Diskon jendela probation per pengesahan sah, dalam basis poin.
pub const ENDORSEMENT_PROBATION_DISCOUNT_BPS: u64 = 2_500;

/// Pembagi lantai jendela probation: jendela tidak pernah lebih pendek dari 1/4 dasar.
pub const PROBATION_FLOOR_DIVISOR: u64 = 4;

/// Parameter protokol siklus hidup validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorPolicy {
    /// Panjang jendela probation (V1).
    pub probation_blocks: u64,
    /// Ambang pelanggaran liveness berurutan selama probation sebelum dibatalkan.
    pub probation_max_miss_streak: u64,
    /// Ambang blok terlewat berurutan sebelum ditahan otomatis (V3).
    pub max_missed_blocks: u64,
    /// Masa denda blok `unjail` untuk validator yang ditahan (V3).
    pub unjail_cooldown_blocks: u64,
    /// Masa denda blok pemulihan untuk validator yang ditangguhkan (V3/V5).
    pub suspension_cooldown_blocks: u64,
    /// Kapasitas maksimum himpunan validator aktif (V2).
    pub max_active_validators: usize,
    /// Panjang epoch dalam blok (V2).
    pub epoch_length_blocks: u64,
    /// Kuota endorsement aktif per pengesah (V4).
    pub max_active_endorsements: usize,
    /// Masa kerja minimum pengesah dalam blok (V4).
    pub min_endorser_tenure_blocks: u64,
}

impl Default for ValidatorPolicy {
    fn default() -> Self {
        Self {
            probation_blocks: DEFAULT_PROBATION_WINDOW_BLOCKS,
            probation_max_miss_streak: MAX_MISSED_BLOCKS_THRESHOLD,
            max_missed_blocks: MAX_MISSED_BLOCKS_THRESHOLD,
            unjail_cooldown_blocks: UNJAIL_COOLDOWN_BLOCKS,
            suspension_cooldown_blocks: SUSPENSION_COOLDOWN_BLOCKS,
            max_active_validators: MAX_ACTIVE_VALIDATORS,
            epoch_length_blocks: EPOCH_LENGTH_BLOCKS,
            max_active_endorsements: MAX_ACTIVE_ENDORSEMENTS,
            min_endorser_tenure_blocks: MIN_ENDORSER_TENURE_BLOCKS,
        }
    }
}

impl ValidatorPolicy {
    /// Validasi kebijakan agar ambang tidak pernah bernilai nol/mustahil dipenuhi.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::InvalidValidatorPolicy` berisi alasan penolakan.
    pub fn validate(&self) -> Result<(), ValidatorError> {
        let checks: [(bool, &'static str); 8] = [
            (
                self.probation_blocks > 0,
                "panjang jendela probation harus lebih dari nol",
            ),
            (
                self.probation_max_miss_streak > 0,
                "ambang pelanggaran probation harus lebih dari nol",
            ),
            (
                self.max_missed_blocks > 0,
                "ambang blok terlewat harus lebih dari nol",
            ),
            (
                self.unjail_cooldown_blocks > 0,
                "masa denda unjail harus lebih dari nol",
            ),
            (
                self.suspension_cooldown_blocks >= self.unjail_cooldown_blocks,
                "masa denda penangguhan tidak boleh lebih pendek dari masa denda penahanan",
            ),
            (
                self.max_active_validators > 0,
                "kapasitas himpunan aktif harus lebih dari nol",
            ),
            (
                self.epoch_length_blocks > 0,
                "panjang epoch harus lebih dari nol",
            ),
            (
                self.max_active_endorsements > 0,
                "kuota endorsement harus lebih dari nol",
            ),
        ];
        for (valid, reason) in checks {
            if !valid {
                return Err(ValidatorError::InvalidValidatorPolicy { reason });
            }
        }
        Ok(())
    }

    /// Jadwal batas epoch untuk kebijakan ini.
    #[must_use]
    pub const fn schedule(&self) -> EpochSchedule {
        EpochSchedule::new(self.epoch_length_blocks)
    }
}

/// Hasil rotasi himpunan validator pada batas epoch (V2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochRotation {
    /// Epoch tujuan hasil rotasi.
    pub epoch: u64,
    /// Ketinggian blok batas epoch tempat rotasi dijalankan.
    pub boundary_height: u64,
    /// Himpunan aktif baru, terurut menurut peringkat.
    pub active_set: Vec<AccountId>,
    /// Validator yang naik ke `ActiveSet` pada rotasi ini.
    pub promoted: Vec<AccountId>,
    /// Validator yang turun kembali ke `Eligible` pada rotasi ini.
    pub demoted: Vec<AccountId>,
    /// Peringkat penuh kandidat: `(akun, skor BPS-terbobot)`.
    pub ranking: Vec<(AccountId, u64)>,
}

/// Hasil pemotongan stake denda (*slashing*) (V4/V5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashOutcome {
    /// Akun yang dikenai pemotongan.
    pub account: AccountId,
    /// Jumlah Quanta yang dipotong (pembulatan bilangan bulat ke bawah).
    pub slashed_quanta: u64,
    /// Sisa stake setelah pemotongan.
    pub remaining_stake_quanta: u64,
    /// Jumlah endorsement yang dicabut karena pengesah dinodai.
    pub endorsements_revoked: usize,
    /// `true` bila pemotongan memicu penangguhan otomatis.
    pub suspended: bool,
}

/// Mesin siklus hidup validator (V0–V5).
///
/// Menyatukan gerbang penerimaan berbasis peran/stake/PoP, pipeline probation,
/// rotasi himpunan aktif deterministik per epoch, pelacakan kesalahan keaktifan
/// dengan penahanan otomatis, kuota endorsement anti-Sybil, serta akuntansi
/// stake berbasis basis poin tanpa *floating-point*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorLifecycle {
    records: BTreeMap<AccountId, ValidatorRecord>,
    consensus_keys: BTreeSet<PublicKeyBytes>,
    endorsements: EndorsementLedger,
    policy: ValidatorPolicy,
}

impl ValidatorLifecycle {
    /// Bangun mesin siklus hidup dengan kebijakan tervalidasi.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::InvalidValidatorPolicy` bila kebijakan memuat
    /// ambang nol atau kombinasi yang mustahil dipenuhi.
    pub fn new(policy: ValidatorPolicy) -> Result<Self, ValidatorError> {
        policy.validate()?;
        Ok(Self {
            records: BTreeMap::new(),
            consensus_keys: BTreeSet::new(),
            endorsements: EndorsementLedger::new(policy.max_active_endorsements),
            policy,
        })
    }

    /// Kebijakan protokol yang berlaku pada mesin ini.
    #[must_use]
    pub const fn policy(&self) -> &ValidatorPolicy {
        &self.policy
    }

    /// Jumlah rekaman validator terdaftar (termasuk probation/retired).
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// `true` bila belum ada rekaman validator terdaftar.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Rekaman validator untuk satu akun berdaulat.
    #[must_use]
    pub fn record(&self, account: &AccountId) -> Option<&ValidatorRecord> {
        self.records.get(account)
    }

    /// Seluruh rekaman validator dalam urutan kanonikal `AccountId`.
    #[must_use = "iterator tidak memindai apa pun bila tidak dikonsumsi"]
    pub fn records(&self) -> impl Iterator<Item = (&AccountId, &ValidatorRecord)> {
        self.records.iter()
    }

    /// Himpunan validator berstatus `ActiveSet`.
    #[must_use]
    pub fn active_set(&self) -> BTreeSet<AccountId> {
        self.records
            .iter()
            .filter(|(_, record)| record.status.counts_toward_quorum())
            .map(|(account, _)| *account)
            .collect()
    }

    /// Jumlah validator berstatus `ActiveSet`.
    #[must_use]
    pub fn active_validator_count(&self) -> usize {
        self.records
            .values()
            .filter(|record| record.status.counts_toward_quorum())
            .count()
    }

    /// Kumpulan kunci master akun (`DeviceRole::Master`) yang tercatat.
    fn master_keys_of(account: &SovereignAccount) -> BTreeSet<PublicKeyBytes> {
        account
            .devices
            .iter()
            .filter(|(_, record)| record.role == DeviceRole::Master)
            .map(|(key, _)| *key)
            .collect()
    }

    /// Terima kandidat validator ke masa percobaan (V0).
    ///
    /// Gerbang penerimaan memeriksa berurutan: belum terdaftar, peran akun
    /// `ValidatorCandidate`, komitmen stake minimum, bukti kepemilikan kunci
    /// konsensus yang terikat pada akun, dan kunci konsensus belum dipakai
    /// validator lain. Seluruh penolakan bersifat atomik: tidak ada satu pun
    /// bidang state yang berubah.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::AlreadyRegistered`,
    /// `ValidatorError::UnauthorizedRole`, `ValidatorError::InsufficientStake`,
    /// `ValidatorError::InvalidProofOfPossession`, atau
    /// `ValidatorError::DuplicateConsensusKey`.
    pub fn register(
        &mut self,
        account: &SovereignAccount,
        proof: &ProofOfPossession,
        current_block: u64,
    ) -> Result<(), ValidatorError> {
        let account_id = account.account_id;
        if self.records.contains_key(&account_id) {
            return Err(ValidatorError::AlreadyRegistered(account_id));
        }
        if account.role != Role::ValidatorCandidate {
            return Err(ValidatorError::UnauthorizedRole {
                expected: Role::ValidatorCandidate,
                actual: account.role,
            });
        }
        if account.staked_quanta < MIN_VALIDATOR_STAKE_QUANTA {
            return Err(ValidatorError::InsufficientStake {
                provided: account.staked_quanta,
                required: MIN_VALIDATOR_STAKE_QUANTA,
            });
        }
        proof.verify(&account_id)?;
        if self.consensus_keys.contains(&proof.consensus_pubkey) {
            return Err(ValidatorError::DuplicateConsensusKey(
                proof.consensus_pubkey,
            ));
        }

        let record = ValidatorRecord::new(
            account_id,
            proof.consensus_pubkey,
            Self::master_keys_of(account),
            account.staked_quanta,
            current_block,
        );
        self.consensus_keys.insert(proof.consensus_pubkey);
        self.records.insert(account_id, record);
        tracing::info!(
            account = ?account_id,
            stake_quanta = account.staked_quanta,
            current_block,
            "Validator diterima dan masuk masa probation"
        );
        Ok(())
    }

    /// Sinkronkan ulang kunci master akun setelah rotasi kunci berdaulat.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::NotRegistered` bila akun belum terdaftar.
    pub fn sync_master_keys(&mut self, account: &SovereignAccount) -> Result<(), ValidatorError> {
        let account_id = account.account_id;
        let master_keys = Self::master_keys_of(account);
        let record = self
            .records
            .get_mut(&account_id)
            .ok_or(ValidatorError::NotRegistered(account_id))?;
        record.master_keys = master_keys;
        Ok(())
    }

    /// Laporkan hasil satu blok dan tegakkan pelacakan keaktifan (V1/V3).
    ///
    /// * `Probation` + blok bertanda tangan: meluluskan otomatis ke `Eligible`
    ///   begitu syarat blok jendela terpenuhi.
    /// * `Probation` + blok terlewat: mencatat pelanggaran liveness dan
    ///   memulai ulang jendela probation dari blok ini.
    /// * `ActiveSet` + blok terlewat berurutan mencapai `max_missed_blocks`:
    ///   menahan otomatis ke `Jailed` (keluar dari kuorum seketika).
    /// * Status lain dilaporkan sebagai `NotActiveInConsensus` tanpa mutasi.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::NotRegistered`,
    /// `ValidatorError::NotActiveInConsensus`, `ValidatorError::ArithmeticOverflow`,
    /// atau `ValidatorError::ProbationFailed`. Pembatalan probation adalah
    /// satu-satunya jalur yang memindahkan status ke `Retired` sambil
    /// mengembalikan galat, sebab pembatalan itu sendiri adalah mutasi terminal
    /// yang diminta — bukan penolakan tanpa efek.
    pub fn report_block(
        &mut self,
        account: &AccountId,
        current_block: u64,
        signed: bool,
    ) -> Result<BlockReport, ValidatorError> {
        let required = self.probation_requirement(account);
        let miss_limit = self.policy.probation_max_miss_streak;
        let jail_limit = self.policy.max_missed_blocks;
        let record = self
            .records
            .get_mut(account)
            .ok_or(ValidatorError::NotRegistered(*account))?;

        match record.status {
            ValidatorStatus::Probation if !signed => {
                record.probation_miss_streak = record
                    .probation_miss_streak
                    .checked_add(1)
                    .ok_or(ValidatorError::ArithmeticOverflow)?;
                record.lifetime_missed_blocks = record
                    .lifetime_missed_blocks
                    .checked_add(1)
                    .ok_or(ValidatorError::ArithmeticOverflow)?;
                // Pelanggaran liveness: jendela probation dimulai ulang dari blok ini.
                record.probation_start_block = current_block;
                record.probation_blocks_observed = 0;
                record.probation_signed_blocks = 0;
                let miss_streak = record.probation_miss_streak;
                if miss_streak >= miss_limit {
                    record.status = ValidatorStatus::Retired;
                    return Err(ValidatorError::ProbationFailed {
                        misses: miss_streak,
                        observed_blocks: 0,
                    });
                }
                Ok(BlockReport::ProbationWindowReset { miss_streak })
            }
            ValidatorStatus::Probation => {
                record.probation_miss_streak = 0;
                record.probation_blocks_observed = record
                    .probation_blocks_observed
                    .checked_add(1)
                    .ok_or(ValidatorError::ArithmeticOverflow)?;
                record.probation_signed_blocks = record
                    .probation_signed_blocks
                    .checked_add(1)
                    .ok_or(ValidatorError::ArithmeticOverflow)?;
                record.lifetime_signed_blocks = record
                    .lifetime_signed_blocks
                    .checked_add(1)
                    .ok_or(ValidatorError::ArithmeticOverflow)?;
                let observed = record.probation_blocks_observed;
                if observed >= required {
                    record.status = ValidatorStatus::Eligible;
                    tracing::info!(
                        account = ?record.account,
                        observed,
                        required,
                        "Validator lulus probation dan menjadi eligible"
                    );
                    return Ok(BlockReport::Graduated {
                        height: current_block,
                    });
                }
                Ok(BlockReport::ProbationProgress { observed, required })
            }
            ValidatorStatus::ActiveSet if signed => {
                record.consecutive_missed_blocks = 0;
                record.lifetime_signed_blocks = record
                    .lifetime_signed_blocks
                    .checked_add(1)
                    .ok_or(ValidatorError::ArithmeticOverflow)?;
                Ok(BlockReport::Signed {
                    consecutive_missed_blocks: 0,
                })
            }
            ValidatorStatus::ActiveSet => {
                record.lifetime_missed_blocks = record
                    .lifetime_missed_blocks
                    .checked_add(1)
                    .ok_or(ValidatorError::ArithmeticOverflow)?;
                let missed = record
                    .consecutive_missed_blocks
                    .checked_add(1)
                    .ok_or(ValidatorError::ArithmeticOverflow)?;
                record.consecutive_missed_blocks = missed;
                if missed >= jail_limit {
                    record.status = ValidatorStatus::Jailed;
                    record.jail_start_block = current_block;
                    record.suspension_reason = None;
                    tracing::warn!(
                        account = ?record.account,
                        missed,
                        current_block,
                        "Validator ditahan otomatis karena melewati ambang blok terlewat"
                    );
                    return Ok(BlockReport::AutoJailed {
                        consecutive_missed_blocks: missed,
                        height: current_block,
                    });
                }
                Ok(BlockReport::MissRecorded {
                    consecutive_missed_blocks: missed,
                })
            }
            other => Err(ValidatorError::NotActiveInConsensus { status: other }),
        }
    }

    /// Luluskan validator dari `Probation` ke `Eligible` (V1).
    ///
    /// Idempoten untuk validator yang sudah `Eligible`, sehingga simpul yang
    /// tidak melaporkan setiap blok tetap dapat memutakhirkan statusnya selama
    /// catatan jendela probation utuh.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::NotRegistered`,
    /// `ValidatorError::ProbationNotFinished` bila jendela belum tuntas,
    /// `ValidatorError::InvalidStatusTransition` bila status bukan
    /// `Probation`/`Eligible`, dan `ValidatorError::InsufficientStake` bila stake
    /// turun di bawah ambang minimum.
    pub fn graduate(&mut self, account: &AccountId) -> Result<(), ValidatorError> {
        let required = self.probation_requirement(account);
        let record = self
            .records
            .get_mut(account)
            .ok_or(ValidatorError::NotRegistered(*account))?;
        match record.status {
            ValidatorStatus::Eligible => Ok(()),
            ValidatorStatus::Probation => {
                let observed = record.probation_blocks_observed;
                if observed < required {
                    return Err(ValidatorError::ProbationNotFinished { observed, required });
                }
                if record.stake_quanta < MIN_VALIDATOR_STAKE_QUANTA {
                    return Err(ValidatorError::InsufficientStake {
                        provided: record.stake_quanta,
                        required: MIN_VALIDATOR_STAKE_QUANTA,
                    });
                }
                record.status = ValidatorStatus::Eligible;
                Ok(())
            }
            other => Err(ValidatorError::InvalidStatusTransition {
                from: other,
                to: ValidatorStatus::Eligible,
            }),
        }
    }

    /// Aktifkan validator `Eligible` menjadi anggota `ActiveSet`.
    ///
    /// Hanya dilayani pada batas epoch dan wajib melewati status `Eligible`:
    /// promosi paksa dari `Probation`, `Jailed`, atau `Suspended` ditolak tanpa
    /// mengubah state apa pun.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::NotRegistered`,
    /// `ValidatorError::EpochNotBoundary` bila blok bukan batas epoch, dan
    /// `ValidatorError::InvalidStatusTransition` bila status bukan `Eligible`.
    pub fn activate(
        &mut self,
        account: &AccountId,
        current_block: u64,
    ) -> Result<(), ValidatorError> {
        if !self.policy.schedule().is_boundary(current_block) {
            return Err(ValidatorError::EpochNotBoundary {
                height: current_block,
            });
        }
        let record = self
            .records
            .get_mut(account)
            .ok_or(ValidatorError::NotRegistered(*account))?;
        if record.status != ValidatorStatus::Eligible {
            return Err(ValidatorError::InvalidStatusTransition {
                from: record.status,
                to: ValidatorStatus::ActiveSet,
            });
        }
        record.status = ValidatorStatus::ActiveSet;
        record.active_since_block = current_block;
        Ok(())
    }

    /// Pratinjau seleksi himpunan aktif pada batas epoch tanpa mengubah state.
    ///
    /// Berguna bagi simpul untuk memverifikasi hasil seleksi deterministik dan
    /// bagi verifikator untuk membandingkan peringkat secara byte-per-byte.
    ///
    /// # Errors
    /// Mengembalikan galat bertipe dari `select_active_set`: bukan batas epoch,
    /// kapasitas tidak valid, atau luapan aritmetika skor.
    pub fn selection_preview(
        &self,
        current_block: u64,
    ) -> Result<ActiveSetSelection, ValidatorError> {
        select_active_set(
            self.policy.schedule(),
            current_block,
            &self.records,
            self.policy.max_active_validators,
        )
    }

    /// Rotasi himpunan validator aktif pada batas epoch (V2).
    ///
    /// Seleksi dihitung deterministik dari stake dan skor kinerja, dibatasi
    /// `max_active_validators`, lalu diterapkan secara atomik: validator terpilih
    /// naik ke `ActiveSet`, sedangkan anggota lama yang tidak terpilih kembali
    /// turun ke `Eligible` (bukan `Jailed`) sehingga tetap dapat mengikuti rotasi
    /// berikutnya.
    ///
    /// # Errors
    /// Mengembalikan galat bertipe dari [`ValidatorLifecycle::selection_preview`].
    pub fn rotate_epoch(&mut self, current_block: u64) -> Result<EpochRotation, ValidatorError> {
        let selection = self.selection_preview(current_block)?;
        let selected: BTreeSet<AccountId> = selection.active_set.iter().copied().collect();
        let mut promoted: Vec<AccountId> = Vec::new();
        let mut demoted: Vec<AccountId> = Vec::new();

        for (account, record) in &mut self.records {
            let was_active = record.status == ValidatorStatus::ActiveSet;
            if selected.contains(account) {
                if !was_active {
                    record.status = ValidatorStatus::ActiveSet;
                    record.active_since_block = current_block;
                    promoted.push(*account);
                }
            } else if was_active {
                record.status = ValidatorStatus::Eligible;
                record.active_since_block = 0;
                demoted.push(*account);
            }
        }

        promoted.sort_unstable();
        demoted.sort_unstable();
        tracing::info!(
            epoch = selection.epoch,
            boundary_height = current_block,
            active = selection.active_set.len(),
            promoted = promoted.len(),
            demoted = demoted.len(),
            "Rotasi himpunan validator aktif dieksekusi"
        );

        Ok(EpochRotation {
            epoch: selection.epoch,
            boundary_height: current_block,
            active_set: selection.active_set,
            promoted,
            demoted,
            ranking: selection.ranking,
        })
    }

    /// Proses permohonan pemulihan validator yang ditahan/ditangguhkan (V3).
    ///
    /// Wajib memenuhi masa denda blok, ditandatangani kunci master akun yang
    /// terdaftar, dan memakai nonce `unjail` yang benar. Nonce naik atomik
    /// setelah sukses, sehingga tanda tangan yang sama tidak dapat diputar ulang.
    /// Seluruh penolakan bersifat atomik.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::NotRegistered`,
    /// `ValidatorError::InvalidStatusTransition` bila status bukan
    /// `Jailed`/`Suspended`, `ValidatorError::InvalidUnjailAuthorization` bila
    /// binding akun/nonce/tanda tangan tidak sah, dan
    /// `ValidatorError::UnjailCooldownActive` bila masa denda belum selesai.
    pub fn request_unjail(
        &mut self,
        request: &UnjailRequest,
        current_block: u64,
    ) -> Result<(), ValidatorError> {
        {
            let record = self
                .records
                .get(&request.account)
                .ok_or(ValidatorError::NotRegistered(request.account))?;
            let cooldown = match record.status {
                ValidatorStatus::Jailed => self.policy.unjail_cooldown_blocks,
                ValidatorStatus::Suspended => self.policy.suspension_cooldown_blocks,
                other => {
                    return Err(ValidatorError::InvalidStatusTransition {
                        from: other,
                        to: ValidatorStatus::Eligible,
                    })
                }
            };
            if !record.is_master_key(&request.master_key) {
                return Err(ValidatorError::InvalidUnjailAuthorization);
            }
            request.verify(&record.account, record.unjail_nonce)?;
            let elapsed = current_block.saturating_sub(record.jail_start_block);
            if elapsed < cooldown {
                return Err(ValidatorError::UnjailCooldownActive {
                    remaining: cooldown - elapsed,
                });
            }
        }

        let record = self
            .records
            .get_mut(&request.account)
            .ok_or(ValidatorError::NotRegistered(request.account))?;
        let advanced = record
            .unjail_nonce
            .checked_add(1)
            .ok_or(ValidatorError::ArithmeticOverflow)?;
        record.unjail_nonce = advanced;
        record.status = ValidatorStatus::Eligible;
        record.consecutive_missed_blocks = 0;
        record.jail_start_block = 0;
        record.suspension_reason = None;
        tracing::info!(account = ?request.account, current_block, "Validator dipulihkan ke status eligible");
        Ok(())
    }

    /// Tangguhkan validator karena putusan dewan atau pelanggaran berat (V3/V5).
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::NotRegistered`, atau
    /// `ValidatorError::InvalidStatusTransition` bila status asal tidak boleh
    /// berpindah ke `Suspended` (mis. validator masih `Probation` atau sudah
    /// `Retired`); state tidak berubah pada jalur penolakan.
    pub fn suspend(
        &mut self,
        account: &AccountId,
        reason: SuspensionReason,
        current_block: u64,
    ) -> Result<(), ValidatorError> {
        let record = self
            .records
            .get_mut(account)
            .ok_or(ValidatorError::NotRegistered(*account))?;
        record
            .status
            .authorize_transition(ValidatorStatus::Suspended)?;
        record.status = ValidatorStatus::Suspended;
        record.suspension_reason = Some(reason);
        record.jail_start_block = current_block;
        record.active_since_block = 0;
        tracing::warn!(account = ?account, reason = ?reason, "Validator ditangguhkan");
        Ok(())
    }

    /// Pensiunkan validator secara permanen.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::NotRegistered` atau
    /// `ValidatorError::InvalidStatusTransition` bila status sudah `Retired`.
    pub fn retire(&mut self, account: &AccountId) -> Result<(), ValidatorError> {
        let record = self
            .records
            .get_mut(account)
            .ok_or(ValidatorError::NotRegistered(*account))?;
        record
            .status
            .authorize_transition(ValidatorStatus::Retired)?;
        record.status = ValidatorStatus::Retired;
        record.active_since_block = 0;
        Ok(())
    }

    /// Status tujuan setelah pemotongan berat.
    ///
    /// `Suspended` bila transisinya sah dari status saat ini, `Retired` bila
    /// tidak (mis. validator masih `Probation`), dan `None` bila status sudah
    /// terminal sehingga hanya stake yang terpengaruh.
    fn post_slash_status(status: ValidatorStatus) -> Option<ValidatorStatus> {
        if status.can_transition_to(ValidatorStatus::Suspended) {
            Some(ValidatorStatus::Suspended)
        } else if status.can_transition_to(ValidatorStatus::Retired) {
            Some(ValidatorStatus::Retired)
        } else {
            None
        }
    }

    /// Potong stake validator berbasis BPS dan nodai endorsement-nya bila berat (V4/V5).
    ///
    /// Pemotongan memakai pembagian bilangan bulat sehingga sisanya selalu
    /// menguntungkan pemilik stake (tidak ada pembulatan ke atas), dan seluruh
    /// pemeriksaan galat diselesaikan sebelum mutasi diterapkan, sehingga jalur
    /// penolakan tidak pernah menyisakan state setengah jadi.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::NotRegistered`,
    /// `ValidatorError::InvalidSlashRate` bila tarif melebihi `10.000` BPS, serta
    /// `ValidatorError::ArithmeticOverflow` bila perkalian/akumulasi meluap.
    pub fn slash(
        &mut self,
        account: &AccountId,
        rate_bps: u64,
        current_block: u64,
    ) -> Result<SlashOutcome, ValidatorError> {
        let (amount, remaining, slashed_total, status_before) = {
            let record = self
                .records
                .get(account)
                .ok_or(ValidatorError::NotRegistered(*account))?;
            let amount = slash_amount(record.stake_quanta, rate_bps)?;
            let remaining = record
                .stake_quanta
                .checked_sub(amount)
                .ok_or(ValidatorError::ArithmeticOverflow)?;
            let slashed_total = record
                .slashed_quanta
                .checked_add(amount)
                .ok_or(ValidatorError::ArithmeticOverflow)?;
            (amount, remaining, slashed_total, record.status)
        };

        let severe = rate_bps >= SEVERE_SLASH_RATE_BPS;
        let target_status = if severe {
            Self::post_slash_status(status_before)
        } else {
            None
        };
        // Penodaan endorsement pengesah yang dihukum berat (V4).
        let revoked = if severe {
            self.endorsements.taint_endorser(account)
        } else {
            0
        };

        let record = self
            .records
            .get_mut(account)
            .ok_or(ValidatorError::NotRegistered(*account))?;
        record.stake_quanta = remaining;
        record.slashed_quanta = slashed_total;
        if let Some(status) = target_status {
            record.status = status;
            record.active_since_block = 0;
            if status == ValidatorStatus::Suspended {
                record.jail_start_block = current_block;
                record.suspension_reason = Some(SuspensionReason::SevereSlashing);
            }
        }
        tracing::warn!(
            account = ?account,
            rate_bps,
            slashed_quanta = amount,
            remaining_stake_quanta = remaining,
            endorsements_revoked = revoked,
            "Stake validator dipotong"
        );

        Ok(SlashOutcome {
            account: *account,
            slashed_quanta: amount,
            remaining_stake_quanta: remaining,
            endorsements_revoked: revoked,
            suspended: target_status == Some(ValidatorStatus::Suspended),
        })
    }

    /// Berikan endorsement kepada kandidat baru (V4).
    ///
    /// Pengesah wajib validator `ActiveSet` dengan masa kerja minimum, target
    /// wajib kandidat (`Probation`/`Eligible`), dan kuota endorsement aktif
    /// pengesah tidak boleh dilampaui.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::SelfEndorsementForbidden`,
    /// `ValidatorError::NotRegistered`, `ValidatorError::NotActiveInConsensus`,
    /// `ValidatorError::EndorserTenureTooShort`,
    /// `ValidatorError::EndorsementTargetNotEligible`,
    /// `ValidatorError::DuplicateEndorsement`, `ValidatorError::EndorserTainted`,
    /// atau `ValidatorError::EndorsementQuotaExceeded`.
    pub fn endorse(
        &mut self,
        endorser: &AccountId,
        candidate: &AccountId,
        current_block: u64,
    ) -> Result<(), ValidatorError> {
        if endorser == candidate {
            return Err(ValidatorError::SelfEndorsementForbidden);
        }
        let min_tenure = self.policy.min_endorser_tenure_blocks;
        {
            let record = self
                .records
                .get(endorser)
                .ok_or(ValidatorError::NotRegistered(*endorser))?;
            if record.status != ValidatorStatus::ActiveSet {
                return Err(ValidatorError::NotActiveInConsensus {
                    status: record.status,
                });
            }
            let tenure = record.tenure_blocks(current_block);
            if tenure < min_tenure {
                return Err(ValidatorError::EndorserTenureTooShort {
                    tenure,
                    required: min_tenure,
                });
            }
        }
        {
            let record = self
                .records
                .get(candidate)
                .ok_or(ValidatorError::NotRegistered(*candidate))?;
            if !matches!(
                record.status,
                ValidatorStatus::Probation | ValidatorStatus::Eligible
            ) {
                return Err(ValidatorError::EndorsementTargetNotEligible {
                    status: record.status,
                });
            }
        }
        self.endorsements.grant(endorser, candidate, current_block)
    }

    /// Bobot endorsement efektif seorang kandidat (anti-Sybil, V4).
    #[must_use]
    pub fn endorsement_weight(&self, candidate: &AccountId) -> u64 {
        self.endorsements.anti_sybil_weight(candidate)
    }

    /// Jumlah endorsement aktif yang diterbitkan seorang pengesah.
    #[must_use]
    pub fn endorsement_count(&self, endorser: &AccountId) -> usize {
        self.endorsements.active_count(endorser)
    }

    /// `true` bila pengesah sudah dicabut kewenangannya (*tainted*).
    #[must_use]
    pub fn is_endorser_tainted(&self, endorser: &AccountId) -> bool {
        self.endorsements.is_tainted(endorser)
    }

    /// Syarat blok probation efektif untuk seorang kandidat (V1/V4).
    ///
    /// Setiap pengesahan sah (bobot anti-Sybil) memotong jendela sebesar
    /// `ENDORSEMENT_PROBATION_DISCOUNT_BPS`, dengan lantai
    /// `1/PROBATION_FLOOR_DIVISOR` dari jendela dasar. Seluruh aritmetika
    /// bilangan bulat; pasangan timbal balik dan endorsement yang dinodai tidak
    /// mempercepat kelulusan.
    #[must_use]
    pub fn probation_requirement(&self, account: &AccountId) -> u64 {
        let base = self.policy.probation_blocks;
        let floor = base / PROBATION_FLOOR_DIVISOR;
        let weight = self.endorsements.anti_sybil_weight(account);
        let discount = ENDORSEMENT_PROBATION_DISCOUNT_BPS
            .saturating_mul(weight)
            .min(BPS_DENOMINATOR);
        let kept_bps = BPS_DENOMINATOR.saturating_sub(discount);
        let scaled = base
            .checked_mul(kept_bps)
            .map_or(base, |value| value / BPS_DENOMINATOR);
        scaled.max(floor)
    }

    /// Total bobot suara himpunan aktif: akumulasi stake efektif (V5).
    ///
    /// Stake efektif setiap anggota `ActiveSet` adalah
    /// `stake * performance_bps / 10.000`, diakumulasi dengan `checked_add`
    /// sehingga luapan selalu menjadi galat bertipe.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::ArithmeticOverflow` bila akumulasi meluap
    /// atau skor kinerja tidak dapat dihitung.
    pub fn quorum_weight(&self) -> Result<u64, ValidatorError> {
        let mut total: u64 = 0;
        for record in self.records.values() {
            if record.status.counts_toward_quorum() {
                let weight =
                    effective_stake_quanta(record.stake_quanta, record.performance_bps()?)?;
                total = accumulate_weight(total, weight)?;
            }
        }
        Ok(total)
    }

    /// Total stake seluruh rekaman terdaftar (termasuk probation/jailed).
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::ArithmeticOverflow` bila akumulasi meluap.
    pub fn total_stake_quanta(&self) -> Result<u64, ValidatorError> {
        self.records.values().try_fold(0_u64, |total, record| {
            accumulate_weight(total, record.stake_quanta)
        })
    }

    /// Komitmen digest kanonikal seluruh state siklus hidup validator.
    ///
    /// Iterasi memakai urutan `BTreeMap`, sehingga dua simpul dengan isi state
    /// identik selalu menghasilkan digest identik tanpa bergantung pada urutan
    /// pendaftaran.
    #[must_use]
    pub fn state_digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_VALIDATOR_LIFECYCLE_V1");
        for (account, record) in &self.records {
            hasher.update(account);
            hasher.update(&record.state_digest());
        }
        hasher.update(&self.endorsements.digest());
        *hasher.finalize().as_bytes()
    }
}
