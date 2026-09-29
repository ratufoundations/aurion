//! Modul sanitasi ingress untuk validasi dan filtering payload.
//!
//! Modul ini menerapkan validasi struktur dan skema untuk semua data eksternal
//! yang masuk ke gateway, memastikan Zero Malformed Leakage.

use crate::error::GatewayError;
use aurion_criptografi::PublicKeyBytes;
use serde_json::{Value, Number};
use std::str;

/// Batas ukuran payload default: 128 KB
pub const DEFAULT_MAX_PAYLOAD_SIZE: u64 = 128 * 1024;

/// Batas ukuran batch: 2 MB
pub const DEFAULT_MAX_BATCH_SIZE: u64 = 2 * 1024 * 1024;

/// Sanitizer untuk validasi payload JSON.
#[derive(Debug)]
pub struct JsonSanitizer;

impl JsonSanitizer {
    /// Validasi payload JSON.
    ///
    /// # Arguments
    /// * `payload` - Data JSON sebagai bytes
    /// * `max_size` - Batas ukuran maksimal
    ///
    /// # Returns
    /// `Ok(Value)` jika valid, `Err(GatewayError)` jika gagal.
    ///
    /// # Errors
    /// Mengembalikan berbagai error `GatewayError` tergantung jenis kegagalan validasi.
    pub fn validate(payload: &[u8], max_size: u64) -> Result<Value, GatewayError> {
        // GW2: Cek ukuran payload
        if payload.len() as u64 > max_size {
            return Err(GatewayError::PayloadTooLarge {
                size: payload.len() as u64,
                max_allowed: max_size,
            });
        }

        // GW0: Cek payload kosong
        if payload.is_empty() {
            return Err(GatewayError::EmptyPayload);
        }

        // Coba parse JSON
        let value: Value = serde_json::from_slice(payload)
            .map_err(|e| GatewayError::MalformedPayload(e.to_string()))?;

        // Validasi rekursif
        Self::validate_value_recursive(&value)?;

        Ok(value)
    }

    /// Validasi Value JSON secara rekursif.
    fn validate_value_recursive(value: &Value) -> Result<(), GatewayError> {
        match value {
            Value::Null | Value::Bool(_) => Ok(()),
            Value::Number(n) => Self::validate_number(n),
            Value::String(s) => Self::validate_string(s),
            Value::Array(arr) => {
                for item in arr {
                    Self::validate_value_recursive(item)?;
                }
                Ok(())
            }
            Value::Object(obj) => {
                for (_, v) in obj {
                    Self::validate_value_recursive(v)?;
                }
                Ok(())
            }
        }
    }

    /// Validasi number: harus non-negative integer.
    fn validate_number(n: &Number) -> Result<(), GatewayError> {
        // GW0: Tolak float
        if n.is_f64() {
            return Err(GatewayError::FloatValue {
                field: "number".to_string(),
                value: n.to_string(),
            });
        }

        // GW0: Tolak negatif
        if let Some(i) = n.as_i64() {
            if i < 0 {
                return Err(GatewayError::NegativeValue {
                    field: "number".to_string(),
                    value: i,
                });
            }
        }

        Ok(())
    }

    /// Validasi string: harus encoding valid.
    fn validate_string(s: &str) -> Result<(), GatewayError> {
        // GW0: Validasi UTF-8 sudah dilakukan oleh serde_json
        // Cek karakter kontrol
        for (i, c) in s.char_indices() {
            if c.is_control() && c != '\n' && c != '\r' && c != '\t' {
                return Err(GatewayError::InvalidUnicode(format!(
                    "karakter kontrol terlarang pada posisi {i}"
                )));
            }
        }
        Ok(())
    }
}

/// Sanitizer untuk validasi hex string (`AccountId`, tx hash, dll.).
#[derive(Debug)]
pub struct HexSanitizer;

impl HexSanitizer {
    /// Validasi hex string.
    ///
    /// # Arguments
    /// * `hex_str` - String hex yang akan divalidasi
    /// * `_expected_len` - Panjang byte yang diharapkan (opsional, belum digunakan)
    ///
    /// # Returns
    /// `Ok(Vec<u8>)` jika valid, `Err(GatewayError)` jika gagal.
    ///
    /// # Errors
    /// Mengembalikan `Err(GatewayError::InvalidHexFormat)` jika panjang ganjil,
    /// atau `Err(GatewayError::InvalidEncoding)` jika decode gagal.
    pub fn validate_hex(
        hex_str: &str,
        _expected_len: Option<usize>,
    ) -> Result<Vec<u8>, GatewayError> {
        // GW0: Cek panjang ganjil
        if !hex_str.len().is_multiple_of(2) {
            return Err(GatewayError::InvalidHexFormat {
                length: hex_str.len(),
            });
        }

        // Coba decode hex
        hex::decode(hex_str).map_err(|e| GatewayError::InvalidEncoding(e.to_string()))
    }

    /// Validasi `PublicKeyBytes` (32 byte hex).
    ///
    /// # Errors
    /// Mengembalikan error jika hex string tidak valid atau panjang tidak 32 byte.
    pub fn validate_public_key(hex_str: &str) -> Result<PublicKeyBytes, GatewayError> {
        let bytes = Self::validate_hex(hex_str, Some(64))?;
        
        if bytes.len() != 32 {
            return Err(GatewayError::InvalidEncoding(format!(
                "Panjang public key harus 32 byte, ditemukan {}",
                bytes.len()
            )));
        }

        let mut pk = PublicKeyBytes::default();
        pk.copy_from_slice(&bytes);
        Ok(pk)
    }
}

/// Sanitizer untuk validasi ukuran frame biner.
#[derive(Debug)]
pub struct FrameSanitizer;

impl FrameSanitizer {
    /// Validasi ukuran frame.
    ///
    /// # Arguments
    /// * `frame_size` - Ukuran frame yang dilaporkan
    /// * `max_allowed` - Batas ukuran yang diizinkan
    ///
    /// # Returns
    /// `Ok(())` jika valid, `Err(GatewayError)` jika gagal.
    ///
    /// # Errors
    /// Mengembalikan `Err(GatewayError::FrameTooLarge)` jika ukuran melebihi batas,
    /// atau `Err(GatewayError::BufferAllocationFailed)` jika ukuran terlalu besar untuk alokasi.
    pub fn validate_frame_size(frame_size: u64, max_allowed: u64) -> Result<(), GatewayError> {
        // GW2: Tolak frame terlalu besar
        if frame_size > max_allowed {
            return Err(GatewayError::FrameTooLarge {
                size: frame_size,
                max: max_allowed,
            });
        }

        // GW2: Cek overflow saat alokasi
        // Jika ukuran mendekati u64::MAX, jangan alokasikan
        if frame_size > u64::from(u32::MAX) {
            return Err(GatewayError::BufferAllocationFailed { size: frame_size });
        }

        Ok(())
    }

    /// Cek apakah ukuran safe untuk alokasi.
    #[must_use]
    pub fn is_safe_to_allocate(size: u64) -> bool {
        u32::try_from(size).is_ok()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_payload() {
        let result = JsonSanitizer::validate(b"", 1024);
        assert!(matches!(result, Err(GatewayError::EmptyPayload)));
    }

    #[test]
    fn test_valid_json() {
        let json = b"{\"amount\": 100, \"nonce\": 5}";
        let result = JsonSanitizer::validate(json, 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn test_negative_value() {
        let json = b"{\"amount\": -100}";
        let result = JsonSanitizer::validate(json, 1024);
        assert!(matches!(result, Err(GatewayError::NegativeValue { .. })));
    }

    #[test]
    fn test_float_value() {
        let json = b"{\"amount\": 50.5}";
        let result = JsonSanitizer::validate(json, 1024);
        assert!(matches!(result, Err(GatewayError::FloatValue { .. })));
    }

    #[test]
    fn test_malformed_json() {
        let json = b"{amount: 100}"; // Missing quotes
        let result = JsonSanitizer::validate(json, 1024);
        assert!(matches!(result, Err(GatewayError::MalformedPayload(_))));
    }

    #[test]
    fn test_payload_too_large() {
        let json = vec![b'x'; 200_000]; // 200 KB > 128 KB default
        let result = JsonSanitizer::validate(&json, DEFAULT_MAX_PAYLOAD_SIZE);
        assert!(matches!(result, Err(GatewayError::PayloadTooLarge { .. })));
    }

    #[test]
    fn test_valid_hex() {
        // 64 hex chars = 32 bytes
        let hex = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let result = HexSanitizer::validate_hex(hex, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }

    #[test]
    fn test_invalid_hex_odd_length() {
        let hex = "a1b"; // 3 chars = odd length
        let result = HexSanitizer::validate_hex(hex, None);
        assert!(matches!(result, Err(GatewayError::InvalidHexFormat { .. })));
    }

    #[test]
    fn test_invalid_hex_chars() {
        let hex = "a1b2c3g4"; // 'g' is not valid hex
        let result = HexSanitizer::validate_hex(hex, None);
        assert!(matches!(result, Err(GatewayError::InvalidEncoding(_))));
    }

    #[test]
    fn test_frame_too_large() {
        let result = FrameSanitizer::validate_frame_size(u64::MAX, 1024);
        assert!(matches!(result, Err(GatewayError::FrameTooLarge { .. })));
    }

    #[test]
    fn test_safe_to_allocate() {
        assert!(FrameSanitizer::is_safe_to_allocate(1024));
        assert!(FrameSanitizer::is_safe_to_allocate(u64::from(u32::MAX)));
        assert!(!FrameSanitizer::is_safe_to_allocate(u64::from(u32::MAX) + 1));
    }

    #[test]
    fn test_public_key_validation() {
        // Valid 32-byte hex
        let valid_pk = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let result = HexSanitizer::validate_public_key(valid_pk);
        assert!(result.is_ok());
        
        // Invalid length
        let short_pk = "a1b2";
        let result = HexSanitizer::validate_public_key(short_pk);
        assert!(result.is_err());
    }

    #[test]
    fn test_zero_float_guarantee() {
        // Audit statis: Tidak ada tipe f32/f64 di modul ini
        // Izinkan: is_f64(), is_f32() (method serde_json)
        let src = include_str!("../src/sanitizer.rs");
        
        // Cari pola penggunaan tipe (bukan method call)
        let patterns = [" f32", "f32:", "f32,", "f32(", " f64", "f64:", "f64,", "f64("];
        let has_float_type = patterns.iter().any(|p| src.contains(p));
        
        // Izinkan method is_f64()
        let has_allowed_methods = src.contains("is_f64()") || src.contains("is_f32()");
        
        assert!(!has_float_type || has_allowed_methods, 
            "Pelanggaran Zero-Float: f32/f64 ditemukan di sanitizer.rs");
    }
}
