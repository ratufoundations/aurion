use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum VmError {
    #[error("Kehabisan kuota gas (Out of Gas)")]
    OutOfGas,

    #[error("Stack underflow: operasi membutuhkan elemen yang tidak tersedia di stack")]
    StackUnderflow,

    #[error("Stack overflow: kedalaman stack melebihi batas aman ({0})")]
    StackOverflow(usize),

    #[error("Pointer instruksi (Program Counter) di luar jangkauan kode: {0}")]
    InstructionPointerOutOfBounds(usize),

    #[error("Pembagian dengan nol dilarang")]
    DivisionByZero,

    #[error("Terjadi luapan aritmatika (arithmetic overflow)")]
    ArithmeticOverflow,

    #[error("Eksekusi kontrak memanggil REVERT eksplisit dengan kode status {0}")]
    ExplicitRevert(u64),

    #[error("Alamat kontrak tidak valid atau tidak ditemukan")]
    ContractNotFound,
}
