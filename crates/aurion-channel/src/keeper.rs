use aurion_core::types::Quanta;
use aurion_criptografi::{verifikasi_tanda_tangan, SignatureBytes};
use aurion_execution::{
    AccountKeeper, ExecutionError, Keeper, ModuleId, NamespaceStore, StoreKey, TransactionalCache,
};

use crate::error::ChannelError;
use crate::types::{AccountId, BalanceProof, ChannelId, ChannelState, ChannelStatus};

const CHANNEL_ID_DOMAIN_TAG: &[u8] = b"AURION_CHANNEL_ID_V1";
const CHANNEL_ESCROW_DOMAIN_TAG: &[u8] = b"AURION_CHANNEL_ESCROW_V1";

/// Keeper native untuk micropayment state channel.
///
/// Seluruh mutasi nilai moneter mengalir lewat `AccountKeeper` (pemilik
/// namespace `account`); partisi `channel` hanya menyimpan record lifecycle
/// saluran di bawah StoreKey miliknya sendiri (CH5 / RFC-001).
#[derive(Debug)]
pub struct ChannelKeeper {
    store_key: StoreKey,
}

impl ChannelKeeper {
    pub fn new() -> Result<Self, ExecutionError> {
        Ok(Self {
            store_key: StoreKey::new("channel")?,
        })
    }

    #[must_use]
    pub fn store_key(&self) -> &StoreKey {
        &self.store_key
    }

    fn channel_key(channel_id: ChannelId) -> Vec<u8> {
        let mut k = Vec::with_capacity(5 + 8);
        k.extend_from_slice(b"chan:");
        k.extend_from_slice(&channel_id);
        k
    }

    /// Turunan deterministik channel id dari parameter open:
    /// `blake3("AURION_CHANNEL_ID_V1" || sender || receiver || deposit_le || blocks_le)[..8]`.
    #[must_use]
    pub fn derive_channel_id(
        sender: &AccountId,
        receiver: &AccountId,
        deposit: Quanta,
        challenge_blocks: u64,
    ) -> ChannelId {
        let mut payload = Vec::with_capacity(CHANNEL_ID_DOMAIN_TAG.len() + 32 + 32 + 16 + 8);
        payload.extend_from_slice(CHANNEL_ID_DOMAIN_TAG);
        payload.extend_from_slice(sender);
        payload.extend_from_slice(receiver);
        payload.extend_from_slice(&deposit.to_le_bytes());
        payload.extend_from_slice(&challenge_blocks.to_le_bytes());
        let digest = aurion_criptografi::Hasher::digest(&payload);
        let mut id = [0u8; 8];
        id.copy_from_slice(&digest[..8]);
        id
    }

    /// Address semu escrow: `blake3("AURION_CHANNEL_ESCROW_V1" || channel_id)[..32]`.
    #[must_use]
    pub fn escrow_account(channel_id: ChannelId) -> AccountId {
        let mut payload = Vec::with_capacity(CHANNEL_ESCROW_DOMAIN_TAG.len() + 8);
        payload.extend_from_slice(CHANNEL_ESCROW_DOMAIN_TAG);
        payload.extend_from_slice(&channel_id);
        aurion_criptografi::Hasher::digest(&payload)
    }

    fn load_state(
        store: &mut NamespaceStore<'_>,
        channel_id: ChannelId,
    ) -> Result<ChannelState, ChannelError> {
        let key = Self::channel_key(channel_id);
        match store.get(&key)? {
            Some(bytes) => ChannelState::decode(&bytes),
            None => Err(ChannelError::ChannelNotFound(channel_id)),
        }
    }

    /// Membaca state saluran dari namespace `channel` (read-only).
    ///
    /// # Errors
    /// Mengembalikan `ChannelError::ChannelNotFound` bila saluran tidak ada.
    pub fn get_channel(
        &self,
        cache: &mut TransactionalCache,
        channel_id: ChannelId,
    ) -> Result<ChannelState, ChannelError> {
        let mut store = NamespaceStore::new(self.store_key.clone(), cache);
        Self::load_state(&mut store, channel_id)
    }

    /// Fase Open (CH1): mengunci `deposit` dari saldo sender secara atomik ke
    /// escrow saluran dan menulis record channel pada namespace `channel`.
    ///
    /// # Errors
    /// Mengembalikan `ChannelError` bila saluran sudah ada, saldo sender
    /// tidak mencukupi, atau operasi storage/aritmetika gagal — seluruh
    /// mutasi overlay dibatalkan (rollback total).
    pub fn open_channel(
        &self,
        cache: &mut TransactionalCache,
        sender: &AccountId,
        receiver: &AccountId,
        deposit: Quanta,
        challenge_blocks: u64,
    ) -> Result<ChannelState, ChannelError> {
        let account = AccountKeeper::new()?;
        let channel_id = Self::derive_channel_id(sender, receiver, deposit, challenge_blocks);
        let escrow = Self::escrow_account(channel_id);

        // Saluran ganda dengan id turunan sama ditolak sebelum debit.
        {
            let mut store = NamespaceStore::new(self.store_key.clone(), cache);
            let key = Self::channel_key(channel_id);
            if store.get(&key)?.is_some() {
                return Err(ChannelError::ChannelAlreadyExists(channel_id));
            }
        }

        // Debit sender -> escrow via AccountKeeper (atomic CoW overlay).
        {
            let mut account_store = NamespaceStore::new(account.store_key().clone(), cache);
            account.transfer(&mut account_store, sender, &escrow, deposit)?;
        }

        let state = ChannelState {
            channel_id,
            sender: *sender,
            receiver: *receiver,
            total_deposit: deposit,
            settled_amount: 0,
            last_nonce: 0,
            challenge_period: challenge_blocks,
            status: ChannelStatus::Open,
        };

        {
            let mut store = NamespaceStore::new(self.store_key.clone(), cache);
            let key = Self::channel_key(channel_id);
            store.set(&key, &state.encode())?;
        }

        Ok(state)
    }

    /// Penutupan sepihak: validasi tiket lalu membuka jendela sanggah
    /// `challenge_period` blok. Tiket nonce lebih tinggi yang masuk saat
    /// saluran sedang `Challenging` langsung menyanggah (supersede) klaim
    /// lama dan memperbarui batas penyelesaian (invarian CH4).
    ///
    /// # Errors
    /// Mengembalikan `ChannelError` untuk signature/nonce/escrow rusak.
    pub fn submit_close(
        &self,
        cache: &mut TransactionalCache,
        proof: &BalanceProof,
        current_height: u64,
    ) -> Result<ChannelState, ChannelError> {
        let mut state = {
            let mut store = NamespaceStore::new(self.store_key.clone(), cache);
            Self::load_state(&mut store, proof.channel_id)?
        };

        if state.status == ChannelStatus::Settled {
            return Err(ChannelError::ChannelSettled);
        }

        Self::validate(proof, &state)?;

        state.settled_amount = proof.transferred_amount;
        state.last_nonce = proof.nonce;

        state.status = match state.status {
            ChannelStatus::Open => ChannelStatus::Challenging {
                expire_height: current_height
                    .checked_add(state.challenge_period)
                    .ok_or(ChannelError::ArithmeticOverflow)?,
            },
            ChannelStatus::Challenging { expire_height } => {
                ChannelStatus::Challenging { expire_height }
            }
            ChannelStatus::Settled => return Err(ChannelError::ChannelSettled),
        };

        Self::persist(&self.store_key, cache, &state)?;
        Ok(state)
    }

    /// Penyelesaian bersama: kedua pihak (sender via tiket, receiver via
    /// `receiver_signature` atas preimage identik) menyetujui distribusi
    /// final. Escrow dibuka langsung tanpa jendela sanggah (CH2).
    ///
    /// # Errors
    /// Mengembalikan `ChannelError` bila salah satu tanda tangan tidak sah.
    pub fn close_cooperatively(
        &self,
        cache: &mut TransactionalCache,
        proof: &BalanceProof,
        receiver_signature: &SignatureBytes,
    ) -> Result<ChannelState, ChannelError> {
        let mut state = {
            let mut store = NamespaceStore::new(self.store_key.clone(), cache);
            Self::load_state(&mut store, proof.channel_id)?
        };

        Self::validate(proof, &state)?;

        let payload = proof.preimage(&state.sender);
        verifikasi_tanda_tangan(&state.receiver, &payload, receiver_signature)
            .map_err(|_| ChannelError::InvalidSignature)?;

        Self::distribute(cache, &state, proof.transferred_amount)?;

        state.settled_amount = proof.transferred_amount;
        state.last_nonce = proof.nonce;
        state.status = ChannelStatus::Settled;
        Self::persist(&self.store_key, cache, &state)?;
        Ok(state)
    }

    /// Finalisasi escrow setelah jendela sanggah berakhir (CH2, CH4):
    /// kredit `settled_amount` ke receiver, refund sisa ke sender.
    ///
    /// # Errors
    /// Mengembalikan `ChallengePeriodActive` bila jendela belum habis.
    pub fn settle_after_challenge(
        &self,
        cache: &mut TransactionalCache,
        channel_id: ChannelId,
        current_height: u64,
    ) -> Result<ChannelState, ChannelError> {
        let mut state = {
            let mut store = NamespaceStore::new(self.store_key.clone(), cache);
            Self::load_state(&mut store, channel_id)?
        };

        match state.status {
            ChannelStatus::Settled => return Err(ChannelError::ChannelSettled),
            ChannelStatus::Open => return Err(ChannelError::NotInChallengeWindow),
            ChannelStatus::Challenging { expire_height } => {
                if current_height < expire_height {
                    return Err(ChannelError::ChallengePeriodActive {
                        remaining_blocks: expire_height - current_height,
                    });
                }
            }
        }

        Self::distribute(cache, &state, state.settled_amount)?;
        state.status = ChannelStatus::Settled;
        Self::persist(&self.store_key, cache, &state)?;
        Ok(state)
    }

    fn validate(proof: &BalanceProof, state: &ChannelState) -> Result<(), ChannelError> {
        if proof.transferred_amount > state.total_deposit {
            return Err(ChannelError::TransferExceedsDeposit {
                requested: proof.transferred_amount,
                deposit: state.total_deposit,
            });
        }
        if proof.nonce <= state.last_nonce {
            return Err(ChannelError::StaleNonce {
                provided: proof.nonce,
                current: state.last_nonce,
            });
        }
        let payload = proof.preimage(&state.sender);
        verifikasi_tanda_tangan(&state.sender, &payload, &proof.signature)
            .map_err(|_| ChannelError::InvalidSignature)?;
        Ok(())
    }

    fn distribute(
        cache: &mut TransactionalCache,
        state: &ChannelState,
        receiver_payout: Quanta,
    ) -> Result<(), ChannelError> {
        if receiver_payout > state.total_deposit {
            return Err(ChannelError::TransferExceedsDeposit {
                requested: receiver_payout,
                deposit: state.total_deposit,
            });
        }
        let sender_refund = state
            .total_deposit
            .checked_sub(receiver_payout)
            .ok_or(ChannelError::ArithmeticOverflow)?;
        let escrow = Self::escrow_account(state.channel_id);
        let account = AccountKeeper::new()?;

        let mut account_store = NamespaceStore::new(account.store_key().clone(), cache);
        account.transfer(
            &mut account_store,
            &escrow,
            &state.receiver,
            receiver_payout,
        )?;
        account.transfer(&mut account_store, &escrow, &state.sender, sender_refund)?;
        Ok(())
    }

    fn persist(
        store_key: &StoreKey,
        cache: &mut TransactionalCache,
        state: &ChannelState,
    ) -> Result<(), ChannelError> {
        let mut store = NamespaceStore::new(store_key.clone(), cache);
        let key = {
            let mut k = Vec::with_capacity(5 + 8);
            k.extend_from_slice(b"chan:");
            k.extend_from_slice(&state.channel_id);
            k
        };
        store.set(&key, &state.encode())?;
        Ok(())
    }
}

impl Keeper for ChannelKeeper {
    fn store_key(&self) -> &StoreKey {
        &self.store_key
    }
    fn module_id(&self) -> ModuleId {
        ModuleId::from_store_key(&self.store_key)
    }
    fn name(&self) -> &'static str {
        "channel"
    }
}
