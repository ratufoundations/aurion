#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Invarian mesin micropayment state channel: CH0–CH5.
//! Seluruh nilai moneter bertipe `Quanta` (u128) — tidak ada floating-point.

use aurion_channel::{
    BalanceProof, ChannelError, ChannelKeeper, ChannelState, ChannelStatus, TICKET_SIZE,
};
use aurion_core::types::Quanta;
use aurion_criptografi::{Keypair, PublicKeyBytes};
use aurion_execution::{AccountKeeper, ExecutionError, Keeper, NamespaceStore, TransactionalCache};

const FUEL_LIMIT: u64 = 1_000_000;
const CHALLENGE_BLOCKS: u64 = 100;

fn alice_kp() -> Keypair {
    Keypair::from_bytes(&[0x01; 32])
}

fn bob_kp() -> Keypair {
    Keypair::from_bytes(&[0x02; 32])
}

fn cache_with_funding(
    funding: &[(PublicKeyBytes, Quanta)],
) -> Result<TransactionalCache, Box<dyn std::error::Error>> {
    let account = AccountKeeper::new()?;
    let mut base = std::collections::BTreeMap::new();
    for (acct, bal) in funding {
        let key = account.store_key().qualify(&account.balance_key(acct));
        base.insert(key, bal.to_be_bytes().to_vec());
    }
    Ok(TransactionalCache::new(base, FUEL_LIMIT))
}

fn read_balance(
    cache: &mut TransactionalCache,
    account: &AccountKeeper,
    acct: &PublicKeyBytes,
) -> Result<Quanta, Box<dyn std::error::Error>> {
    let mut store = NamespaceStore::new(account.store_key().clone(), cache);
    Ok(account.balance(&mut store, acct)?)
}

fn read_escrow(
    cache: &mut TransactionalCache,
    account: &AccountKeeper,
    channel: &ChannelState,
) -> Result<Quanta, Box<dyn std::error::Error>> {
    read_balance(
        cache,
        account,
        &ChannelKeeper::escrow_account(channel.channel_id),
    )
}

fn tick(
    channel_id: [u8; 8],
    nonce: u64,
    transferred: Quanta,
    sender_keypair: &Keypair,
    sender_pub: &PublicKeyBytes,
) -> BalanceProof {
    BalanceProof::signed(channel_id, nonce, transferred, sender_pub, sender_keypair)
}

/// CH0 + CH1: open atomik, idempotensi/double-open, dan penolakan saldo kurang.
#[test]
fn test_ch1_open_atomic_lock_and_rejections() -> Result<(), Box<dyn std::error::Error>> {
    let sender_kp = alice_kp();
    let sender_pub = sender_kp.public_key_bytes();
    let receiver_kp = bob_kp();
    let receiver_pub = receiver_kp.public_key_bytes();

    let mut cache = cache_with_funding(&[(sender_pub, 1_000_000)])?;
    let account = AccountKeeper::new()?;
    let keeper = ChannelKeeper::new()?;

    let channel = keeper.open_channel(
        &mut cache,
        &sender_pub,
        &receiver_pub,
        500_000,
        CHALLENGE_BLOCKS,
    )?;
    assert_eq!(channel.status, ChannelStatus::Open);
    assert_eq!(channel.total_deposit, 500_000);
    assert_eq!(channel.last_nonce, 0);
    assert_eq!(channel.settled_amount, 0);

    // Debit atomik: sender berkurang, escrow terkunci, record channel tersimpan.
    assert_eq!(read_balance(&mut cache, &account, &sender_pub)?, 500_000);
    assert_eq!(read_escrow(&mut cache, &account, &channel)?, 500_000);
    let persisted = keeper.get_channel(&mut cache, channel.channel_id)?;
    assert_eq!(persisted.channel_id, channel.channel_id);
    assert_eq!(ChannelState::decode(&persisted.encode())?, persisted);

    // Idempotensi: open identik ditolak sebelum debit apa pun.
    let dup = keeper.open_channel(
        &mut cache,
        &sender_pub,
        &receiver_pub,
        500_000,
        CHALLENGE_BLOCKS,
    );
    assert!(matches!(dup, Err(ChannelError::ChannelAlreadyExists(_))));

    // Open kedua dengan deposit berbeda tetap valid.
    let second = keeper.open_channel(
        &mut cache,
        &sender_pub,
        &receiver_pub,
        300_000,
        CHALLENGE_BLOCKS,
    )?;
    assert_ne!(second.channel_id, channel.channel_id);
    assert_eq!(read_balance(&mut cache, &account, &sender_pub)?, 200_000);

    // Saldo kurang -> rollback total: saldo dan partisi channel utuh.
    let poor_kp = Keypair::from_bytes(&[0x0D; 32]);
    let poor_pub = poor_kp.public_key_bytes();
    let mut poor_cache = cache_with_funding(&[(poor_pub, 40)])?;
    let err = keeper.open_channel(
        &mut poor_cache,
        &poor_pub,
        &receiver_pub,
        500_000,
        CHALLENGE_BLOCKS,
    );
    assert!(matches!(
        err,
        Err(ChannelError::InsufficientSenderBalance {
            available: 40,
            required: 500_000
        })
    ));
    let poor_account = AccountKeeper::new()?;
    let poor_channel_id =
        ChannelKeeper::derive_channel_id(&poor_pub, &receiver_pub, 500_000, CHALLENGE_BLOCKS);
    assert_eq!(read_balance(&mut poor_cache, &poor_account, &poor_pub)?, 40);
    assert!(matches!(
        keeper.get_channel(&mut poor_cache, poor_channel_id),
        Err(ChannelError::ChannelNotFound(_))
    ));

    Ok(())
}

/// CH0: monotonicity nonce/amount + CH3 verifikasi tiket 128-byte.
#[test]
fn test_ch0_monotonicity_and_ch3_ticket_verifiability() -> Result<(), Box<dyn std::error::Error>> {
    let sender_kp = alice_kp();
    let sender_pub = sender_kp.public_key_bytes();

    let mut cache = cache_with_funding(&[(sender_pub, 1_000_000)])?;
    let keeper = ChannelKeeper::new()?;

    let channel = keeper.open_channel(
        &mut cache,
        &sender_pub,
        &sender_pub,
        500_000,
        CHALLENGE_BLOCKS,
    )?;
    let cid = channel.channel_id;

    let p1 = tick(cid, 1, 100_000, &sender_kp, &sender_pub);
    assert!(p1.verify(&sender_pub)?);
    let p2 = tick(cid, 2, 200_000, &sender_kp, &sender_pub);
    assert!(p2.verify(&sender_pub)?);

    // Monotonicity: amount kumulatif tidak pernah menurun; nonce naik.
    assert!(p2.nonce > p1.nonce);
    assert!(p2.transferred_amount >= p1.transferred_amount);

    // Kanal moneter murni u128: transfer di atas escrow ditolak.
    let over = tick(cid, 3, 600_000, &sender_kp, &sender_pub);
    let res = keeper.submit_close(&mut cache, &over, 10);
    assert!(matches!(
        res,
        Err(ChannelError::TransferExceedsDeposit {
            requested: 600_000,
            deposit: 500_000
        })
    ));

    // Ticket 128-byte: round-trip encode/decode identik.
    let ticket_bytes = p1.encode_ticket(&sender_pub);
    assert_eq!(ticket_bytes.len(), TICKET_SIZE);
    assert_eq!(TICKET_SIZE, 128);
    let (decoded, decoded_pub) = BalanceProof::decode_ticket(&ticket_bytes)?;
    assert_eq!(decoded, p1);
    assert_eq!(decoded_pub, sender_pub);
    assert_eq!(decoded.preimage(&decoded_pub), p1.preimage(&sender_pub));
    assert!(BalanceProof::decode_ticket(&ticket_bytes[..120]).is_err());

    // CH3: satu byte signature diubah -> verify menolak; pubkey salah -> menolak.
    let mut forged = p1.clone();
    forged.signature[37] ^= 0x01;
    assert!(!forged.verify(&sender_pub)?);

    let other_kp = Keypair::from_bytes(&[0xBB; 32]);
    let other_pub = other_kp.public_key_bytes();
    assert!(!p1.verify(&other_pub)?);

    Ok(())
}

/// CH3 (engine): signature/pemalsuan kunci ditolak saat `submit_close`.
#[test]
fn test_ch3_engine_rejects_forged_signature_and_stale_nonce(
) -> Result<(), Box<dyn std::error::Error>> {
    let sender_kp = alice_kp();
    let sender_pub = sender_kp.public_key_bytes();
    let receiver_kp = bob_kp();
    let receiver_pub = receiver_kp.public_key_bytes();

    let mut cache = cache_with_funding(&[(sender_pub, 1_000_000)])?;
    let keeper = ChannelKeeper::new()?;
    let channel = keeper.open_channel(
        &mut cache,
        &sender_pub,
        &receiver_pub,
        500_000,
        CHALLENGE_BLOCKS,
    )?;

    // Public key sender dipalsukan -> InvalidSignature.
    let attacker = BalanceProof::signed(channel.channel_id, 1, 100_000, &[0xEE; 32], &sender_kp);
    let res = keeper.submit_close(&mut cache, &attacker, 10);
    assert!(matches!(res, Err(ChannelError::InvalidSignature)));

    // Nonce tidak naik -> StaleNonce.
    keeper.submit_close(
        &mut cache,
        &tick(channel.channel_id, 1, 100_000, &sender_kp, &sender_pub),
        10,
    )?;
    let replay = tick(channel.channel_id, 1, 100_000, &sender_kp, &sender_pub);
    let res = keeper.submit_close(&mut cache, &replay, 11);
    assert!(matches!(
        res,
        Err(ChannelError::StaleNonce {
            provided: 1,
            current: 1
        })
    ));

    Ok(())
}

/// CH2: konservasi nilai ketat pada penyelesaian bersama.
#[test]
fn test_ch2_cooperative_conservation() -> Result<(), Box<dyn std::error::Error>> {
    let sender_kp = alice_kp();
    let sender_pub = sender_kp.public_key_bytes();
    let receiver_kp = bob_kp();
    let receiver_pub = receiver_kp.public_key_bytes();

    let mut cache = cache_with_funding(&[(sender_pub, 1_000_000)])?;
    let account = AccountKeeper::new()?;
    let keeper = ChannelKeeper::new()?;

    let channel = keeper.open_channel(
        &mut cache,
        &sender_pub,
        &receiver_pub,
        500_000,
        CHALLENGE_BLOCKS,
    )?;
    let latest = tick(channel.channel_id, 2, 450_000, &sender_kp, &sender_pub);
    let receiver_sig = receiver_kp.sign(&latest.preimage(&sender_pub));

    let closed = keeper.close_cooperatively(&mut cache, &latest, &receiver_sig)?;
    assert_eq!(closed.status, ChannelStatus::Settled);
    assert_eq!(closed.last_nonce, 2);
    assert_eq!(closed.settled_amount, 450_000);

    // Konservasi: receiver 450.000 + refund sender 50.000 == deposit 500.000.
    let sender_balance = read_balance(&mut cache, &account, &sender_pub)?;
    let receiver_balance = read_balance(&mut cache, &account, &receiver_pub)?;
    assert_eq!(sender_balance, 550_000);
    assert_eq!(receiver_balance, 450_000);
    assert_eq!(receiver_balance, closed.settled_amount);
    assert_eq!(sender_balance, 1_000_000 - closed.settled_amount);
    assert_eq!(
        read_escrow(&mut cache, &account, &channel)?,
        0,
        "escrow harus kosong setelah settle"
    );

    // Saluran Settled terkunci: seluruh operasi lanjut ditolak.
    assert!(matches!(
        keeper.submit_close(&mut cache, &latest, 20),
        Err(ChannelError::ChannelSettled)
    ));
    assert!(matches!(
        keeper.settle_after_challenge(&mut cache, channel.channel_id, 999),
        Err(ChannelError::ChannelSettled)
    ));

    Ok(())
}

/// CH2 + CH4: sengketa sepihak — gugatan nonce lebih tinggi menyanggah tiket usang.
#[test]
fn test_ch4_unilateral_supersede_and_conservation() -> Result<(), Box<dyn std::error::Error>> {
    let sender_kp = alice_kp();
    let sender_pub = sender_kp.public_key_bytes();
    let receiver_kp = bob_kp();
    let receiver_pub = receiver_kp.public_key_bytes();

    let mut cache = cache_with_funding(&[(sender_pub, 1_000_000)])?;
    let account = AccountKeeper::new()?;
    let keeper = ChannelKeeper::new()?;

    let channel = keeper.open_channel(
        &mut cache,
        &sender_pub,
        &receiver_pub,
        500_000,
        CHALLENGE_BLOCKS,
    )?;
    let cid = channel.channel_id;

    // Penutupan sepihak di height 100 -> Challenging (expire = 200).
    let old = tick(cid, 1, 200_000, &sender_kp, &sender_pub);
    let challenging = keeper.submit_close(&mut cache, &old, 100)?;
    assert!(matches!(
        challenging.status,
        ChannelStatus::Challenging { expire_height: 200 }
    ));

    // Sebelum habis masa sanggah, dana belum boleh dicairkan.
    assert!(matches!(
        keeper.settle_after_challenge(&mut cache, cid, 150),
        Err(ChannelError::ChallengePeriodActive {
            remaining_blocks: 50
        })
    ));

    // Gugatan baru dengan nonce lebih tinggi menyanggah klaim usang (CH4).
    let newest = tick(cid, 2, 450_000, &sender_kp, &sender_pub);
    let superseded = keeper.submit_close(&mut cache, &newest, 180)?;
    assert!(matches!(
        superseded.status,
        ChannelStatus::Challenging { expire_height: 200 }
    ));
    assert_eq!(superseded.last_nonce, 2);
    assert_eq!(superseded.settled_amount, 450_000);

    // Settle tetap menunggu jendela habis, lalu distribusi dengan tiket termutakhir.
    assert!(matches!(
        keeper.settle_after_challenge(&mut cache, cid, 199),
        Err(ChannelError::ChallengePeriodActive {
            remaining_blocks: 1
        })
    ));
    let settled = keeper.settle_after_challenge(&mut cache, cid, 200)?;
    assert_eq!(settled.status, ChannelStatus::Settled);
    assert_eq!(settled.settled_amount, 450_000);

    let sender_balance = read_balance(&mut cache, &account, &sender_pub)?;
    let receiver_balance = read_balance(&mut cache, &account, &receiver_pub)?;
    assert_eq!(sender_balance, 550_000);
    assert_eq!(receiver_balance, 450_000);
    assert_eq!(sender_balance + receiver_balance, 1_000_000);
    assert_eq!(
        read_escrow(&mut cache, &account, &channel)?,
        0,
        "konservasi nilai: tidak ada Quanta tersisa di escrow"
    );

    Ok(())
}

/// CH5: zero-state leakage — keeper channel menolak menulis kunci namespace asing.
#[test]
fn test_ch5_zero_state_leakage() -> Result<(), Box<dyn std::error::Error>> {
    let account = AccountKeeper::new()?;
    let keeper = ChannelKeeper::new()?;
    let receiver_kp = bob_kp();
    let receiver_pub = receiver_kp.public_key_bytes();
    let mut cache = cache_with_funding(&[(receiver_pub, 1_000_000)])?;

    // Hak milik namespace saluran hanya mencakup partisi channel.
    let foreign = account
        .store_key()
        .qualify(&account.balance_key(&receiver_pub));
    assert!(!keeper.store_key().owns(&foreign));
    assert!(keeper
        .store_key()
        .owns(&keeper.store_key().qualify(b"chan:")));

    // write_raw ke kunci milik namespace akun harus ditolak otoritas.
    {
        let mut store = NamespaceStore::new(keeper.store_key().clone(), &mut cache);
        let err = store.write_raw(&foreign, &0u128.to_be_bytes());
        assert!(matches!(
            err,
            Err(ExecutionError::UnauthorizedStoreAccess { .. })
        ));
    }

    // write_raw ke kunci kanonikal milik sendiri diterima (isolasi tetap terjaga).
    let own = keeper.store_key().qualify(b"chan:probe");
    {
        let mut store = NamespaceStore::new(keeper.store_key().clone(), &mut cache);
        store.write_raw(&own, b"probe")?;
        assert_eq!(store.get(b"chan:probe")?, Some(b"probe".to_vec()));
    }

    Ok(())
}
