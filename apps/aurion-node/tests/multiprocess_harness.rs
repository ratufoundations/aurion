#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Klaster multi-proses nyata: empat proses `aurion-node` terpisah berkomunikasi
//! melalui TCP sungguhan (wire codec + gerbang handshake Aurion), lalu seluruh
//! simpul mencapai kuorum BFT 3-of-4 pada kandidat yang sama.
//!
//! Tidak ada mock: proses-proses ini adalah binary lengkap yang menjalankan
//! ledger redb, mempool, guard council, dan driver P2P daemon.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

/// Binary yang diuji (dibangun oleh cargo saat test ini dikompilasi).
const BIN: &str = env!("CARGO_BIN_EXE_aurion-node");
/// Empat simpul validator agar ambang `2f+1 = 3` tercapai.
const NODE_COUNT: usize = 4;
/// Batas waktu keseluruhan klaster meraih kuorum.
const TIMEOUT: Duration = Duration::from_secs(90);

/// Satu proses node beserta saluran stdout/stderr yang sudah dialirkan.
struct ProcNode {
    child: Child,
    stdout: std_mpsc::Receiver<String>,
    stderr: std_mpsc::Receiver<String>,
}

/// Baca `needle` dari saluran; `None` bila proses tutup atau batas waktu habis.
fn recv_until(rx: &std_mpsc::Receiver<String>, needle: &str, deadline: Instant) -> Option<String> {
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => {
                if line.contains(needle) {
                    return Some(line);
                }
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return None,
        }
    }
    None
}

/// Direktori kerja tempat `config/node.dev.toml` didefinisikan (akarl workspace).
fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../")
}

/// Hidupkan satu proses node; dial semua `peers` yang sudah berjalan.
fn spawn_node(identity: usize, peers: &[String]) -> ProcNode {
    let dir = std::env::temp_dir().join(format!("aurion-mproc-{identity}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let mut command = Command::new(BIN);
    command
        .current_dir(workspace_root())
        .arg("--identity")
        .arg(identity.to_string())
        .arg("--roster-size")
        .arg(NODE_COUNT.to_string())
        .arg("--port")
        .arg("0")
        .arg("--data-dir")
        .arg(&dir);
    for peer in peers {
        command.arg("--peer").arg(peer);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().expect("spawn proses aurion-node");
    let stdout_rx = drain_pipe(child.stdout.take().expect("stdout proses"));
    let stderr_rx = drain_pipe(child.stderr.take().expect("stderr proses"));
    ProcNode {
        child,
        stdout: stdout_rx,
        stderr: stderr_rx,
    }
}

/// Alihkan satu pipe menjadi penerima baris di utas terpisah (mencegah
/// buffer penuh memblokir proses anak pada stderr yang tidak dibaca).
fn drain_pipe(reader: impl Read + Send + 'static) -> std_mpsc::Receiver<String> {
    let (tx, rx) = std_mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

#[tokio::test]
async fn m0_four_process_cluster_reaches_bft_quorum_on_real_tcp() {
    let deadline = Instant::now() + TIMEOUT;
    let mut procs: Vec<ProcNode> = Vec::new();
    let mut addresses: Vec<String> = Vec::new();

    // Spawn berjenjang: setiap simpul mendial simpul-simpul yang lebih dulu
    // naik (topologi terarah, tanpa koneksi simetris yang mubazir).
    for identity in 0..NODE_COUNT {
        let proc = spawn_node(identity, &addresses);
        let listen = recv_until(&proc.stdout, "AURION_LISTEN:", deadline).unwrap_or_else(|| {
            let stderr: Vec<String> = proc.stderr.try_iter().collect();
            panic!("node {identity} tidak melaporkan listener; stderr: {stderr:?}");
        });
        let addr = listen
            .strip_prefix("AURION_LISTEN:")
            .unwrap_or_default()
            .to_owned();
        assert!(addr.contains(':'), "format alamat tak sah: {listen}");
        addresses.push(addr.clone());
        tracing::info!(identity, addr, "Simpul klaster naik");
        procs.push(proc);
    }

    // Seluruh simpul harus melaporkan kuorum BFT pada tinggi yang sama.
    for (index, proc) in procs.iter_mut().enumerate() {
        if recv_until(&proc.stdout, "AURION_QUORUM_READY", deadline).is_none() {
            let stderr: Vec<String> = proc.stderr.try_iter().collect();
            panic!("node {index} tidak meraih kuorum; stderr: {stderr:?}");
        }
    }

    // Tidak boleh ada proses yang keluar prematur; tutup sisanya dengan
    // sinyal `kill` lalu reaper semua.
    for (index, proc) in procs.iter_mut().enumerate() {
        match proc.child.try_wait() {
            Ok(Some(status)) => panic!("node {index} keluar sebelum kuorum: {status}"),
            Ok(None) => {}
            Err(error) => panic!("gagal memeriksa node {index}: {error}"),
        }
        let _ = proc.child.kill();
        let _ = proc.child.wait();
    }
}
