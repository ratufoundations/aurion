//! Tipe kanonikal dan konstanta moneter Aurion.
//!
//! Seluruh nilai moneter (saldo, fee, stake, reward, slash) menggunakan
//! `Quanta` (`u128`) dengan presisi `10^-10` AUR per Quanta.

use crate::error::ExecutionError;

/// Satuan moneter kanonikal Aurion: 1 AUR = `QUANTA_PER_AUR` Quanta.
pub type Quanta = u128;

/// Rasio presisi: jumlah Quanta dalam satu AUR (10 desimal / 10^10).
pub const QUANTA_PER_AUR: Quanta = 10_000_000_000;

/// Denominator basis-poin untuk penghitungan rasio BPS (10.000 = 100%).
pub const BPS_DENOMINATOR: u128 = 10_000;

/// Treasury genesis: 66 juta AUR = `660_000_000_000_000_000` Quanta.
pub const TREASURY_GENESIS_QUANTA: Quanta = 660_000_000_000_000_000;

/// Aritmetika Quanta ber-skala presisi: `val * num / den` dengan pembulatan ke bawah (floor).
///
/// # Errors
/// Mengembalikan `DivisionByZero` bila `den == 0` dan `ArithmeticOverflow`
/// bila perkalian bernilai ber-skala meluap melebihi jangkauan `u128`.
pub fn mul_div_quanta(val: Quanta, num: u64, den: u64) -> Result<Quanta, ExecutionError> {
    if den == 0 {
        return Err(ExecutionError::DivisionByZero);
    }
    let scaled = val
        .checked_mul(u128::from(num))
        .ok_or(ExecutionError::ArithmeticOverflow)?;
    scaled
        .checked_div(u128::from(den))
        .ok_or(ExecutionError::ArithmeticOverflow)
}

/// Menghitung bagian proporsional Quanta dalam basis-poin: `amount * bps / 10_000`.
///
/// # Errors
/// Mengembalikan `DivisionByZero` atau `ArithmeticOverflow` bila aritmetika tidak valid.
pub fn calculate_bps(amount: Quanta, bps: u64) -> Result<Quanta, ExecutionError> {
    mul_div_quanta(amount, bps, 10_000)
}
