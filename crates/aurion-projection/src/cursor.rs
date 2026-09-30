//! Modul kursor proyeksi untuk konsumsi delta aliran monotonik.
//!
//! Modul ini menerapkan pelacakan ketinggian blok untuk memastikan konsumsi
//! delta bersifat 100% idempoten dan terurut secara monotonik (P0).

use crate::error::ProjectionError;

/// Nilai default untuk kursor awal.
pub const INITIAL_CURSOR: u64 = 0;

/// Kursor proyeksi yang melacak ketinggian blok terakhir yang diproyeksikan.
///
/// Kursor ini memastikan bahwa:
/// 1. Delta/blok dikonsumsi dalam urutan monotonik (H_next = H_current + 1)
/// 2. Re-applying delta yang sama tidak menyebabkan duplikasi
/// 3. Hash state proyeksi tetap konsisten
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionCursor {
    /// Ketinggian blok terakhir yang berhasil diproyeksikan.
    height: u64,
    /// Hash state proyeksi pada ketinggian tersebut (digest).
    state_digest: [u8; 32],
}

impl ProjectionCursor {
    /// Buat kursor proyeksi baru dengan ketinggian 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            height: INITIAL_CURSOR,
            state_digest: [0u8; 32],
        }
    }

    /// Buat kursor proyeksi dengan ketinggian dan digest awal.
    #[must_use]
    pub fn with_initial(height: u64, state_digest: [u8; 32]) -> Self {
        Self {
            height,
            state_digest,
        }
    }

    /// Dapatkan ketinggian blok terakhir yang diproyeksikan.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Dapatkan hash state proyeksi.
    #[must_use]
    pub fn state_digest(&self) -> &[u8; 32] {
        &self.state_digest
    }

    /// Validasi apakah ketinggian blok berikutnya valid untuk diproyeksikan.
    ///
    /// # Arguments
    /// * `next_height` - Ketinggian blok yang akan diproyeksikan
    ///
    /// # Returns
    /// `Ok(())` jika valid, `Err(ProjectionError)` jika tidak.
    ///
    /// # Errors
    /// - `ProjectionError::NonSequentialBlock` jika blok tidak berurutan
    /// - `ProjectionError::BlockAlreadyProjected` jika blok sudah diproyeksikan
    pub fn validate_next_height(&self, next_height: u64) -> Result<(), ProjectionError> {
        if next_height == self.height {
            // Idempotensi: blok sudah diproyeksikan
            return Err(ProjectionError::BlockAlreadyProjected {
                height: next_height,
            });
        }

        if next_height != self.height + 1 {
            // Blok tidak berurutan
            return Err(ProjectionError::NonSequentialBlock {
                expected: self.height + 1,
                got: next_height,
            });
        }

        Ok(())
    }

    /// Perbarui kursor ke ketinggian berikutnya dengan digest state baru.
    ///
    /// # Arguments
    /// * `next_height` - Ketinggian blok yang baru saja diproyeksikan
    /// * `new_digest` - Hash state proyeksi yang baru
    ///
    /// # Returns
    /// `Ok(())` jika berhasil, `Err(ProjectionError)` jika validasi gagal.
    pub fn advance(
        &mut self,
        next_height: u64,
        new_digest: [u8; 32],
    ) -> Result<(), ProjectionError> {
        self.validate_next_height(next_height)?;
        self.height = next_height;
        self.state_digest = new_digest;
        Ok(())
    }

    /// Cek apakah kursor sudah diinisialisasi (height > 0).
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.height != INITIAL_CURSOR
    }

    /// Reset kursor ke keadaan awal.
    pub fn reset(&mut self) {
        self.height = INITIAL_CURSOR;
        self.state_digest = [0u8; 32];
    }
}

impl Default for ProjectionCursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_creation() {
        let cursor = ProjectionCursor::new();
        assert_eq!(cursor.height(), 0);
        assert!(!cursor.is_initialized());
    }

    #[test]
    fn test_cursor_advance_sequential() {
        let mut cursor = ProjectionCursor::new();

        // Advance to height 1
        cursor
            .advance(1, [1u8; 32])
            .expect("should advance to height 1");
        assert_eq!(cursor.height(), 1);

        // Advance to height 2
        cursor
            .advance(2, [2u8; 32])
            .expect("should advance to height 2");
        assert_eq!(cursor.height(), 2);

        // Advance to height 3
        cursor
            .advance(3, [3u8; 32])
            .expect("should advance to height 3");
        assert_eq!(cursor.height(), 3);
    }

    #[test]
    fn test_cursor_reject_non_sequential() {
        let mut cursor = ProjectionCursor::new();
        cursor.advance(1, [1u8; 32]).unwrap();

        // Try to jump to height 3 (skipping 2)
        let result = cursor.advance(3, [3u8; 32]);
        assert!(matches!(
            result,
            Err(ProjectionError::NonSequentialBlock {
                expected: 2,
                got: 3
            })
        ));

        // Cursor should still be at height 1
        assert_eq!(cursor.height(), 1);
    }

    #[test]
    fn test_cursor_reject_duplicate() {
        let mut cursor = ProjectionCursor::new();
        cursor.advance(1, [1u8; 32]).unwrap();

        // Try to apply height 1 again
        let result = cursor.advance(1, [1u8; 32]);
        assert!(matches!(
            result,
            Err(ProjectionError::BlockAlreadyProjected { height: 1 })
        ));

        // Cursor should still be at height 1
        assert_eq!(cursor.height(), 1);
    }

    #[test]
    fn test_cursor_state_digest_update() {
        let mut cursor = ProjectionCursor::new();
        let digest_1 = [1u8; 32];
        let digest_2 = [2u8; 32];

        cursor.advance(1, digest_1).unwrap();
        assert_eq!(cursor.state_digest(), &digest_1);

        cursor.advance(2, digest_2).unwrap();
        assert_eq!(cursor.state_digest(), &digest_2);
    }

    #[test]
    fn test_cursor_reset() {
        let mut cursor = ProjectionCursor::with_initial(5, [5u8; 32]);
        assert_eq!(cursor.height(), 5);

        cursor.reset();
        assert_eq!(cursor.height(), 0);
        assert!(!cursor.is_initialized());
    }

    #[test]
    fn test_cursor_with_initial() {
        let digest = [42u8; 32];
        let cursor = ProjectionCursor::with_initial(100, digest);
        assert_eq!(cursor.height(), 100);
        assert_eq!(cursor.state_digest(), &digest);
        assert!(cursor.is_initialized());
    }

    #[test]
    fn test_idempotency_no_state_change() {
        let mut cursor = ProjectionCursor::new();
        let original_digest = [1u8; 32];
        cursor.advance(1, original_digest).unwrap();

        // Try to re-apply the same block
        let result = cursor.validate_next_height(1);
        assert!(matches!(
            result,
            Err(ProjectionError::BlockAlreadyProjected { .. })
        ));

        // State should be unchanged
        assert_eq!(cursor.height(), 1);
        assert_eq!(cursor.state_digest(), &original_digest);
    }
}
