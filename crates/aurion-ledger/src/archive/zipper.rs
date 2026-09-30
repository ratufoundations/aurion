use crate::archive::manifest::EpochManifest;
use crate::archive::packer::PackedArchive;
use crate::error::LedgerError;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

/// Nama entry kanonikal di dalam arsip ZIP.
pub const MANIFEST_ENTRY: &str = "manifest.json";
pub const BLOCKS_ENTRY: &str = "blocks.bin";
pub const CHECKSUM_ENTRY: &str = "checksum.blake3";

/// Menulis arsip ZIP atomik berisi manifest, blocks.bin, dan checksum.
///
/// Arsip ditulis ke file sementara (`.tmp`) lalu di-rename atomik untuk
/// mencegah korupsi partial (AR3).
///
/// # Errors
/// Mengembalikan `LedgerError` jika I/O atau kompresi gagal.
pub fn write_archive(path: &Path, packed: &PackedArchive) -> Result<(), LedgerError> {
    let tmp_path = path.with_extension("tmp");
    let file = fs::File::create(&tmp_path)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal membuat file arsip: {e}")))?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file(MANIFEST_ENTRY, options)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal menulis manifest: {e}")))?;
    zip.write_all(
        packed
            .manifest
            .to_json()
            .map_err(|e| LedgerError::ArchiveError(format!("Gagal serialisasi manifest: {e}")))?
            .as_bytes(),
    )
    .map_err(|e| LedgerError::ArchiveError(format!("Gagal menulis manifest: {e}")))?;

    zip.start_file(BLOCKS_ENTRY, options)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal menulis blocks: {e}")))?;
    zip.write_all(&packed.blocks_bin)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal menulis blocks: {e}")))?;

    zip.start_file(CHECKSUM_ENTRY, options)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal menulis checksum: {e}")))?;
    zip.write_all(&packed.checksum)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal menulis checksum: {e}")))?;

    zip.finish()
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal finalisasi ZIP: {e}")))?;

    fs::rename(&tmp_path, path)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal rename arsip atomik: {e}")))?;

    Ok(())
}

/// Membaca dan memverifikasi arsip ZIP yang sudah ada.
///
/// # Errors
/// Mengembalikan `LedgerError` jika arsip korup atau checksum tidak cocok.
pub fn read_archive(path: &Path) -> Result<(EpochManifest, Vec<u8>, [u8; 32]), LedgerError> {
    let file = fs::File::open(path)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal membuka arsip: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal membaca ZIP: {e}")))?;

    let mut manifest_bytes = Vec::new();
    archive
        .by_name(MANIFEST_ENTRY)
        .map_err(|e| LedgerError::ArchiveError(format!("Entry manifest tidak ditemukan: {e}")))?
        .read_to_end(&mut manifest_bytes)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal membaca manifest: {e}")))?;

    let manifest = EpochManifest::from_json(&String::from_utf8_lossy(&manifest_bytes))
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal deserialisasi manifest: {e}")))?;

    let mut blocks_bin = Vec::new();
    archive
        .by_name(BLOCKS_ENTRY)
        .map_err(|e| LedgerError::ArchiveError(format!("Entry blocks tidak ditemukan: {e}")))?
        .read_to_end(&mut blocks_bin)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal membaca blocks: {e}")))?;

    let mut checksum_bytes = Vec::new();
    archive
        .by_name(CHECKSUM_ENTRY)
        .map_err(|e| LedgerError::ArchiveError(format!("Entry checksum tidak ditemukan: {e}")))?
        .read_to_end(&mut checksum_bytes)
        .map_err(|e| LedgerError::ArchiveError(format!("Gagal membaca checksum: {e}")))?;

    let checksum: [u8; 32] = checksum_bytes
        .try_into()
        .map_err(|_| LedgerError::ArchiveError("Checksum panjang tidak valid".to_string()))?;

    crate::archive::packer::verify_checksum(&blocks_bin, &checksum)?;

    Ok((manifest, blocks_bin, checksum))
}
