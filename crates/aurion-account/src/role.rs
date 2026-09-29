use aurion_criptografi::PublicKeyBytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRole {
    /// Pemilik penuh: bisa mutasi saldo tanpa batas, menambah & mencopot perangkat lain
    Master,
    /// Perangkat harian (Laptop/Tablet): tunduk pada limit transaksi & kuota harian
    DailyOperator,
    /// Perangkat pantau: hanya boleh membaca saldo / menerima pesan tanpa izin transfer
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRecord {
    pub device_key: PublicKeyBytes,
    pub role: DeviceRole,
    pub registered_at: u64,
    pub expires_at: u64,
}

impl DeviceRecord {
    #[must_use]
    pub fn is_valid(&self, current_time: u64) -> bool {
        if self.expires_at == 0 {
            true
        } else {
            current_time <= self.expires_at
        }
    }
}
