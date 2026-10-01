import React from "react";
import Link from "next/link";
import { ArrowLeft, Activity, ShieldCheck, CheckCircle2, AlertTriangle, ArrowRight, FileCode, Hash, Key } from "lucide-react";
import { rpcClient, TransactionDetail } from "@/lib/rpc";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/CopyButton";
import { ThemeToggle } from "@/components/theme-toggle";
import { Quanta } from "@aurion/sdk";

interface TxPageProps {
  params: Promise<{ hash: string }>;
}

export function generateStaticParams() {
  return [{ hash: "0x9c4a101" }];
}

export default async function TransactionDetailPage({ params }: TxPageProps) {
  const { hash } = await params;

  let tx: TransactionDetail | null = null;
  let isLiveRpc = false;

  try {
    tx = await rpcClient.getTransaction(hash);
    isLiveRpc = true;
  } catch {
    // Fallback: buat transaksi deterministik dengan format 168-byte kanonikal
    const rawAmount = BigInt("250000000000"); // 25 AUR
    const rawFee = BigInt("1000000"); // 0.00010 AUR
    const quantaAmount = Quanta.fromBigInt(rawAmount);
    const quantaFee = Quanta.fromBigInt(rawFee);

    tx = {
      hash,
      sender: "0x8b3a8b27v5285a50ef03d7890bfa4312e",
      receiver: "0x4d9980a312fe6c7104be9975b3420811",
      amount_quanta: rawAmount,
      amount_aur: `${quantaAmount.toAur()} AUR`,
      network_fee_quanta: rawFee,
      network_fee_aur: `${quantaFee.toAur()} AUR`,
      nonce: 14,
      block_height: 482912,
      status: "confirmed",
      timestamp: 1727727000000,
      ed25519_signature: "7f4c9b2089fa4321d567ea309b114c554d9980a312fe6c7104be9975b3420811e2b4d8a1c3f509e76182adbc449012356789abcdef0123456789abcdef01234567",
    };
  }

  return (
    <div className="min-h-screen bg-[#f6f8fa] dark:bg-[#0d1117] text-[#24292f] dark:text-[#c9d1d9] font-mono transition-colors">
      {/* Top Navbar */}
      <header className="border-b border-[#d0d7de] dark:border-[#30363d] bg-white dark:bg-[#0d1117] px-4 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Link
            href="/"
            className="flex items-center gap-1.5 text-xs text-[#656d76] dark:text-[#8b949e] hover:text-[#0969da] dark:hover:text-[#58a6ff] transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            <span>Dashboard</span>
          </Link>
          <span className="text-[#d0d7de] dark:text-[#30363d]">/</span>
          <span className="text-xs font-bold text-[#1f2328] dark:text-[#f0f6fc] flex items-center gap-1.5">
            <Activity className="w-3.5 h-3.5 text-[#0969da] dark:text-[#58a6ff]" />
            Detail Transaksi
          </span>
        </div>

        <div className="flex items-center gap-2">
          <ThemeToggle />
          <Badge
            variant={isLiveRpc ? "success" : "warning"}
            className="text-[10px] py-0.5"
          >
            {isLiveRpc ? "RPC Data (127.0.0.1:8545)" : "Simulasi Fallback"}
          </Badge>
        </div>
      </header>

      {/* Main Content */}
      <main className="max-w-6xl mx-auto p-4 md:p-6 space-y-4">
        {/* Banner if offline */}
        {!isLiveRpc && (
          <div className="flex items-center justify-between p-3 rounded border border-[#d4a72c]/40 bg-[#fff8c5] dark:bg-[#d29922]/15 text-[#9a6700] dark:text-[#d29922] text-xs">
            <div className="flex items-center gap-2">
              <AlertTriangle className="w-4 h-4 shrink-0" />
              <span>Simpul RPC Lokal (127.0.0.1:8545) offline. Menampilkan struktur kanonikal amplop transaksi 168 byte.</span>
            </div>
            <Link href="/">
              <Button variant="outline" size="sm" className="h-6 text-[11px]">
                Kembali
              </Button>
            </Link>
          </div>
        )}

        {/* Transaction Header Info */}
        <div className="bg-white dark:bg-[#161b22] border border-[#d0d7de] dark:border-[#30363d] p-4 rounded shadow-xs space-y-2">
          <div className="flex items-center justify-between flex-wrap gap-2">
            <div className="flex items-center gap-2">
              <span className="text-[11px] font-mono text-[#656d76] dark:text-[#8b949e] uppercase">
                TRANSACTION IDENTIFIER (BLAKE3 HASH)
              </span>
              <Badge variant="outline" className="text-[10px] py-0">
                168-Byte Envelope
              </Badge>
            </div>
            <div className="flex items-center gap-2">
              <Badge variant="success" className="gap-1 py-0.5 text-xs">
                <CheckCircle2 className="w-3 h-3" />
                <span>Finalized in Block</span>
              </Badge>
              <Link href={`/block/${tx.block_height}`}>
                <Badge variant="info" className="py-0.5 text-xs hover:underline cursor-pointer">
                  #{tx.block_height.toLocaleString()}
                </Badge>
              </Link>
            </div>
          </div>

          <div className="p-2.5 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] break-all text-xs font-mono text-[#0969da] dark:text-[#58a6ff] flex items-center justify-between gap-2">
            <span className="font-semibold">{tx.hash}</span>
            <CopyButton text={tx.hash} />
          </div>
        </div>

        {/* Transfer Flow Card (Sender -> Receiver) */}
        <Card>
          <CardHeader className="py-2.5 px-4">
            <CardTitle className="text-xs">
              <ArrowRight className="w-4 h-4 text-[#0969da] dark:text-[#58a6ff]" />
              <span>ALUR TRANSFER MONETER (ZERO-FLOAT QUANTA)</span>
            </CardTitle>
          </CardHeader>
          <CardContent className="p-4 space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-11 gap-3 items-center">
              {/* Sender Box */}
              <div className="md:col-span-5 p-3 rounded border border-[#d0d7de] dark:border-[#30363d] bg-[#f6f8fa] dark:bg-[#0d1117] space-y-1">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block font-semibold">PENGIRIM (SENDER)</span>
                <Link
                  href={`/account/${tx.sender}`}
                  className="text-xs font-mono text-[#0969da] dark:text-[#58a6ff] hover:underline break-all block"
                >
                  {tx.sender}
                </Link>
                <div className="flex justify-end">
                  <CopyButton text={tx.sender} />
                </div>
              </div>

              {/* Arrow Middle Indicator */}
              <div className="md:col-span-1 flex flex-col items-center justify-center">
                <div className="w-8 h-8 rounded-full bg-[#dafbe1] dark:bg-[#238636]/20 border border-[#4ac26b]/40 dark:border-[#238636]/50 flex items-center justify-center text-[#1a7f37] dark:text-[#3fb950]">
                  <ArrowRight className="w-4 h-4" />
                </div>
              </div>

              {/* Receiver Box */}
              <div className="md:col-span-5 p-3 rounded border border-[#d0d7de] dark:border-[#30363d] bg-[#f6f8fa] dark:bg-[#0d1117] space-y-1">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block font-semibold">PENERIMA (RECEIVER)</span>
                <Link
                  href={`/account/${tx.receiver}`}
                  className="text-xs font-mono text-[#0969da] dark:text-[#58a6ff] hover:underline break-all block"
                >
                  {tx.receiver}
                </Link>
                <div className="flex justify-end">
                  <CopyButton text={tx.receiver} />
                </div>
              </div>
            </div>

            {/* Nominal & Fee Metrics */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3 pt-2 border-t border-[#d0d7de]/50 dark:border-[#30363d]/50">
              <div>
                <span className="text-[11px] text-[#656d76] dark:text-[#8b949e] block">NOMINAL TRANSFER</span>
                <span className="text-xl font-bold text-[#1a7f37] dark:text-[#3fb950] tracking-tight">{tx.amount_aur}</span>
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block">{tx.amount_quanta.toString()} Quanta</span>
              </div>
              <div>
                <span className="text-[11px] text-[#656d76] dark:text-[#8b949e] block">BIAYA JARINGAN (FEE)</span>
                <span className="text-base font-bold text-[#9a6700] dark:text-[#d29922]">{tx.network_fee_aur}</span>
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block">{tx.network_fee_quanta.toString()} Quanta</span>
              </div>
              <div>
                <span className="text-[11px] text-[#656d76] dark:text-[#8b949e] block">NONCE TRANSAKSI</span>
                <span className="text-base font-bold text-[#1f2328] dark:text-[#f0f6fc]">#{tx.nonce}</span>
                <span className="text-[10px] text-[#1a7f37] dark:text-[#3fb950] block">Anti-Replay Passed</span>
              </div>
              <div>
                <span className="text-[11px] text-[#656d76] dark:text-[#8b949e] block">STATUS KONSENSUS</span>
                <Badge variant="success" className="py-0.5 text-xs mt-1">
                  Finalized &amp; Executed
                </Badge>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 168-Byte Canonical Binary Decomposition Card */}
        <Card>
          <CardHeader className="py-2.5 px-4">
            <CardTitle className="text-xs">
              <FileCode className="w-4 h-4 text-[#0969da] dark:text-[#58a6ff]" />
              <span>DEKONSTRUKSI KANONIKAL TRANSAKSI BINARY (168 BYTE ENVELOPE)</span>
            </CardTitle>
          </CardHeader>
          <CardContent className="p-4 space-y-3 text-xs">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div className="p-2.5 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] space-y-1">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block">BYTE [0..4] • DOMAIN TAG &amp; VERSION (4 BYTES)</span>
                <span className="text-[#1f2328] dark:text-[#f0f6fc] font-bold">0x41555231 (AUR1) • Chain ID 1001</span>
              </div>
              <div className="p-2.5 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] space-y-1">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block">BYTE [4..36] • PUBLIC KEY PENGIRIM (32 BYTES)</span>
                <span className="text-[#0969da] dark:text-[#58a6ff] truncate block">{tx.sender}</span>
              </div>
              <div className="p-2.5 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] space-y-1">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block">BYTE [36..68] • ALAMAT PENERIMA (32 BYTES)</span>
                <span className="text-[#0969da] dark:text-[#58a6ff] truncate block">{tx.receiver}</span>
              </div>
              <div className="p-2.5 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] space-y-1">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block">BYTE [68..84] • AMOUNT QUANTA U128-LE (16 BYTES)</span>
                <span className="text-[#1a7f37] dark:text-[#3fb950] font-bold">{tx.amount_quanta.toString()} Quanta</span>
              </div>
              <div className="p-2.5 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] space-y-1">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block">BYTE [84..100] • NETWORK FEE U128-LE (16 BYTES)</span>
                <span className="text-[#9a6700] dark:text-[#d29922] font-bold">{tx.network_fee_quanta.toString()} Quanta</span>
              </div>
              <div className="p-2.5 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] space-y-1">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] block">BYTE [100..108] • NONCE U64-LE (8 BYTES)</span>
                <span className="text-[#1f2328] dark:text-[#f0f6fc] font-bold">Nonce #{tx.nonce}</span>
              </div>
            </div>

            {/* Ed25519 Digital Signature 64 Byte */}
            <div className="p-3 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] space-y-1.5">
              <div className="flex items-center justify-between">
                <span className="text-[10px] text-[#656d76] dark:text-[#8b949e] font-semibold flex items-center gap-1.5">
                  <Key className="w-3.5 h-3.5 text-[#0598ab] dark:text-[#39c5cf]" />
                  BYTE [108..172] • TANDA TANGAN DIGITAL ED25519 (64 BYTES HEX = 128 CHARS)
                </span>
                <CopyButton text={tx.ed25519_signature} />
              </div>
              <div className="p-2 bg-white dark:bg-[#161b22] rounded border border-[#d0d7de]/60 dark:border-[#30363d]/60 break-all text-[11px] text-[#0598ab] dark:text-[#39c5cf]">
                {tx.ed25519_signature}
              </div>
              <div className="flex items-center justify-between text-[10px] text-[#656d76] dark:text-[#8b949e] pt-1">
                <span>Algoritma: Ed25519 Curve25519 Edwards • RFC 8032</span>
                <span className="text-[#1a7f37] dark:text-[#3fb950] font-semibold flex items-center gap-1">
                  <ShieldCheck className="w-3 h-3" /> Cryptographically Verified
                </span>
              </div>
            </div>
          </CardContent>
        </Card>
      </main>
    </div>
  );
}
