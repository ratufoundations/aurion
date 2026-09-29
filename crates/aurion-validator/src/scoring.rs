use crate::error::ValidatorError;

/// Penyebut basis poin: `10.000 BPS = 100%`.
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Skor kinerja penuh (validator ideal tanpa satu pun blok terlewat).
pub const FULL_PERFORMANCE_BPS: u64 = BPS_DENOMINATOR;

/// Skor keaktifan (*uptime*) dalam basis poin.
///
/// Dihitung murni dengan `checked_mul`/`checked_div` bilangan bulat:
/// `signed_blocks * 10.000 / eligible_blocks`. Tidak ada pembulatan pecahan
/// maupun toleransi *floating-point* di seluruh jalur ini.
///
/// # Errors
/// Mengembalikan `ValidatorError::InvalidBlockAccounting` bila kandidat
/// melaporkan blok bertanda tangan melebihi blok yang memenuhi syarat, dan
/// `ValidatorError::ArithmeticOverflow` bila perkalian BPS meluap.
pub fn uptime_bps(signed_blocks: u64, eligible_blocks: u64) -> Result<u64, ValidatorError> {
    if eligible_blocks == 0 {
        return Ok(FULL_PERFORMANCE_BPS);
    }
    if signed_blocks > eligible_blocks {
        return Err(ValidatorError::InvalidBlockAccounting {
            signed: signed_blocks,
            eligible: eligible_blocks,
        });
    }
    signed_blocks
        .checked_mul(BPS_DENOMINATOR)
        .ok_or(ValidatorError::ArithmeticOverflow)
        .map(|scaled| scaled / eligible_blocks)
}

/// Stake efektif: `stake_quanta * performance_bps / 10.000`.
///
/// Dipakai sebagai bobot suara dan skor peringkat seleksi epoch, sehingga
/// validator berkinerja rendah kehilangan bobot secara proporsional tanpa
/// pernah menyentuh aritmetika pecahan.
///
/// # Errors
/// Mengembalikan `ValidatorError::ArithmeticOverflow` bila perkalian stake
/// dengan BPS meluap.
pub fn effective_stake_quanta(
    stake_quanta: u64,
    performance_bps: u64,
) -> Result<u64, ValidatorError> {
    stake_quanta
        .checked_mul(performance_bps)
        .ok_or(ValidatorError::ArithmeticOverflow)
        .map(|scaled| scaled / BPS_DENOMINATOR)
}

/// Nilai potongan stake denda (*slashing*) berbasis BPS.
///
/// `stake_quanta * slash_rate_bps / 10.000` dengan pembagian bilangan bulat
/// murni; sisa bagi selalu dibulatkan ke bawah (tidak pernah memberatkan
/// pemilik stake melampaui tarif yang ditetapkan).
///
/// # Errors
/// Mengembalikan `ValidatorError::InvalidSlashRate` bila `slash_rate_bps`
/// melebihi `10.000`, dan `ValidatorError::ArithmeticOverflow` bila perkalian
/// meluap.
pub fn slash_amount(stake_quanta: u64, slash_rate_bps: u64) -> Result<u64, ValidatorError> {
    if slash_rate_bps > BPS_DENOMINATOR {
        return Err(ValidatorError::InvalidSlashRate {
            rate_bps: slash_rate_bps,
        });
    }
    stake_quanta
        .checked_mul(slash_rate_bps)
        .ok_or(ValidatorError::ArithmeticOverflow)
        .map(|scaled| scaled / BPS_DENOMINATOR)
}

/// Akumulasi bobot suara dengan `checked_add`.
///
/// # Errors
/// Mengembalikan `ValidatorError::ArithmeticOverflow` bila akumulasi bobot
/// melampaui representasi `u64` (tidak pernah membungkam batas secara diam).
pub fn accumulate_weight(current: u64, addend: u64) -> Result<u64, ValidatorError> {
    current
        .checked_add(addend)
        .ok_or(ValidatorError::ArithmeticOverflow)
}

/// Pemeriksaan kuorum berbasis BPS dengan bilangan bulat murni.
///
/// `true` bila `accumulated * 10.000 >= total * threshold_bps`; luapan atau
/// ambang tidak bermakna menghasilkan `false` (*fail-closed*).
#[must_use]
pub fn meets_bps_quorum(accumulated: u64, total: u64, threshold_bps: u64) -> bool {
    if total == 0 || threshold_bps == 0 || threshold_bps > BPS_DENOMINATOR {
        return false;
    }
    match (
        accumulated.checked_mul(BPS_DENOMINATOR),
        total.checked_mul(threshold_bps),
    ) {
        (Some(lhs), Some(rhs)) => lhs >= rhs,
        _ => false,
    }
}
