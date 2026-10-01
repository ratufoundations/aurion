import React from "react";
import Link from "next/link";
import { ArrowLeft, Box, ShieldCheck, CheckCircle2, AlertTriangle, Layers, Clock, ArrowRight } from "lucide-react";
import { rpcClient, BlockDetail } from "@/lib/rpc";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableHead, TableBody, TableRow, TableCell } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/CopyButton";
import { ThemeToggle } from "@/components/theme-toggle";
import { Quanta } from "@aurion/sdk";

interface BlockPageProps {
  params: Promise<{ height: string }>;
}

export function generateStaticParams() {
  return [{ height: "1" }];
}

export default async function BlockDetailPage({ params }: BlockPageProps) {
  const { height } = await params;
  const numHeight = parseInt(height, 10) || 1;

  let block: BlockDetail | null = null;
  let isLiveRpc = false;

  try {
    block = await rpcClient.getBlock(numHeight);
    isLiveRpc = true;
  } catch {
    // Graceful fallback: buat data blok deterministik
    const fallbackHash = `0x8b3a${(numHeight * 123456789).toString(16).padStart(12, "0")}f03d7890bfa4312e`;
    const fallbackParent = `0x8b3a${((numHeight - 1) * 123456789).toString(16).padStart(12, "0")}f03d7890bfa4312e`;
    const fallbackStateRoot = `0x7e29b4c089fa4321d567ea309b114c55${numHeight.toString(16).padStart(8, "0")}`;
    const fallbackTxRoot = `0x4d9980a312fe6c7104be9975b3420811${numHeight.toString(16).padStart(8, "0")}`;

    const sampleTxs = [
      {
        hash: `0x9c4a${numHeight}01${Date.now().toString(16).slice(-8)}`,
        sender: "0x8b3a8b27v5285a50ef03d7890bfa4312e",
        receiver: "0x4d9980a312fe6c7104be9975b3420811",
        amount_quanta: BigInt("250000000000"), // 25 AUR
        amount_aur: "25.0 AUR",
        network_fee_quanta: BigInt("1000000"),
        network_fee_aur: "0.00010 AUR",
        nonce: 14,
        block_height: numHeight,
        status: "confirmed" as const,
        timestamp: 1727727000000,
        ed25519_signature: "e2b4d8a1c3f509e76182adbc449012356789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      },
      {
        hash: `0x7e29${numHeight}02${Date.now().toString(16).slice(-8)}`,
        sender: "0x7e29b4c089fa4321d567ea309b114c55",
        receiver: "0x8b340b3d353d3b60ff40a6b2d7211d46",
        amount_quanta: BigInt("100000000000"), // 10 AUR
        amount_aur: "10.0 AUR",
        network_fee_quanta: BigInt("1000000"),
        network_fee_aur: "0.00010 AUR",
        nonce: 22,
        block_height: numHeight,
        status: "confirmed" as const,
        timestamp: 1727727000000,
        ed25519_signature: "f1a2b3c4d5e67890123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01234567",
      },
      {
        hash: `0x3fb9${numHeight}03${Date.now().toString(16).slice(-8)}`,
        sender: "0x8b340b3d353d3b60ff40a6b2d7211d46",
        receiver: "0x8b3a8b27v5285a50ef03d7890bfa4312e",
        amount_quanta: BigInt("50000000000"), // 5 AUR
        amount_aur: "5.0 AUR",
        network_fee_quanta: BigInt("1000000"),
        network_fee_aur: "0.00010 AUR",
        nonce: 8,
        block_height: numHeight,
        status: "confirmed" as const,
        timestamp: 1727727000000,
        ed25519_signature: "8899aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899aabb",
      },
    ];

    block = {
      height: numHeight,
      hash: fallbackHash,
      prev_hash: fallbackParent,
      state_root: fallbackStateRoot,
      tx_root: fallbackTxRoot,
      proposer: "0x8b3a8b27v5285a50ef03d7890bfa4312e",
      timestamp: 1727727000000,
      round: 0,
      tx_count: 38,
      size_bytes: 18432,
      quorum_sigs: 139,
      total_validators: 142,
      reward_aur: "2.50 AUR",
      qc_signers: [
        "0x8b3a8b27v5285a50ef03d7890bfa4312e",
        "0x4d9980a312fe6c7104be9975b3420811",
        "0x7e29b4c089fa4321d567ea309b114c55",
        "0x8b340b3d353d3b60ff40a6b2d7211d46",
      ],
      transactions: sampleTxs,
    };
  }

  const quorumRatio = `${block.quorum_sigs}/${block.total_validators}`;
  const quorumPct = `${((block.quorum_sigs / block.total_validators) * 100).toFixed(1)}%`;

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
            <Box className="w-3.5 h-3.5 text-[#0969da] dark:text-[#58a6ff]" />
            Blok #{numHeight.toLocaleString()}
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
              <span>Simpul RPC Lokal (127.0.0.1:8545) offline. Menampilkan struktur kanonikal telemetri blok.</span>
            </div>
            <Link href="/">
              <Button variant="outline" size="sm" className="h-6 text-[11px]">
                Kembali
              </Button>
            </Link>
          </div>
        )}

        {/* Block Header Info */}
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-3 bg-white dark:bg-[#161b22] border border-[#d0d7de] dark:border-[#30363d] p-4 rounded shadow-xs">
          <div>
            <div className="flex items-center gap-2 flex-wrap">
              <h1 className="text-lg md:text-xl font-bold text-[#1f2328] dark:text-[#f0f6fc] tracking-tight">
                RINCIAN BLOK #{numHeight.toLocaleString()}
              </h1>
              <Badge variant="success" className="gap-1 py-0.5 text-xs">
                <CheckCircle2 className="w-3 h-3" />
                <span>BFT Finalized</span>
              </Badge>
              <Badge variant="info" className="py-0.5 text-xs">
                Round {block.round}
              </Badge>
            </div>
            <p className="text-xs text-[#656d76] dark:text-[#8b949e] mt-1 flex items-center gap-2">
              <Clock className="w-3 h-3" />
              <span>Terverifikasi pada konsensus sub-detik • Waktu: {new Date(block.timestamp).toLocaleString("id-ID")}</span>
            </p>
          </div>

          <div className="flex items-center gap-2 self-start md:self-auto">
            <Link href={`/block/${Math.max(1, numHeight - 1)}`}>
              <Button variant="outline" size="sm" className="h-7 text-xs">
                &larr; Prev Blok
              </Button>
            </Link>
            <Link href={`/block/${numHeight + 1}`}>
              <Button variant="outline" size="sm" className="h-7 text-xs">
                Next Blok &rarr;
              </Button>
            </Link>
          </div>
        </div>

        {/* 2-Column Consensus & Crypto Metrics */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* Card 1: Ringkasan Konsensus BFT */}
          <Card>
            <CardHeader className="py-2.5 px-4">
              <CardTitle className="text-xs">
                <ShieldCheck className="w-4 h-4 text-[#1a7f37] dark:text-[#3fb950]" />
                <span>KONSENSUS &amp; QUORUM CERTIFICATE (QC)</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="p-4 space-y-3 text-xs">
              <div className="flex items-center justify-between border-b border-[#d0d7de]/50 dark:border-[#30363d]/50 pb-2">
                <span className="text-[#656d76] dark:text-[#8b949e]">Status Kuorum Supermajoritas (+2/3)</span>
                <span className="font-semibold text-[#1a7f37] dark:text-[#3fb950]">
                  {quorumRatio} ({quorumPct})
                </span>
              </div>
              <div className="flex items-center justify-between border-b border-[#d0d7de]/50 dark:border-[#30363d]/50 pb-2">
                <span className="text-[#656d76] dark:text-[#8b949e]">Proposer Simpul</span>
                <div className="flex items-center gap-1.5">
                  <Link
                    href={`/account/${block.proposer}`}
                    className="text-[#0969da] dark:text-[#58a6ff] hover:underline truncate max-w-[200px]"
                  >
                    {block.proposer}
                  </Link>
                  <CopyButton text={block.proposer} />
                </div>
              </div>
              <div className="flex items-center justify-between border-b border-[#d0d7de]/50 dark:border-[#30363d]/50 pb-2">
                <span className="text-[#656d76] dark:text-[#8b949e]">Imbalan Validator (Reward)</span>
                <span className="font-semibold text-[#9a6700] dark:text-[#d29922]">{block.reward_aur}</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-[#656d76] dark:text-[#8b949e]">Ukuran Header Kanonikal</span>
                <span className="font-semibold text-[#1f2328] dark:text-[#f0f6fc]">116 Byte (Binary Enveloped)</span>
              </div>
            </CardContent>
          </Card>

          {/* Card 2: Komponen Kriptografis */}
          <Card>
            <CardHeader className="py-2.5 px-4">
              <CardTitle className="text-xs">
                <Layers className="w-4 h-4 text-[#0969da] dark:text-[#58a6ff]" />
                <span>KOMPONEN KRIPTOGRAFIS (HASH &amp; STATE ROOT)</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="p-4 space-y-3 text-xs">
              <div className="space-y-1">
                <div className="text-[11px] text-[#656d76] dark:text-[#8b949e]">BLOCK HEADER HASH (BLAKE3)</div>
                <div className="p-2 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] break-all text-[11px] text-[#0969da] dark:text-[#58a6ff] flex items-center justify-between gap-2">
                  <span>{block.hash}</span>
                  <CopyButton text={block.hash} />
                </div>
              </div>
              <div className="space-y-1">
                <div className="text-[11px] text-[#656d76] dark:text-[#8b949e]">PARENT (PREV) HASH</div>
                <div className="p-2 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] break-all text-[11px] text-[#656d76] dark:text-[#8b949e] flex items-center justify-between gap-2">
                  <Link href={`/block/${Math.max(1, numHeight - 1)}`} className="hover:underline text-[#0969da] dark:text-[#58a6ff]">
                    {block.prev_hash}
                  </Link>
                  <CopyButton text={block.prev_hash} />
                </div>
              </div>
              <div className="space-y-1">
                <div className="text-[11px] text-[#656d76] dark:text-[#8b949e]">STATE ROOT (MERKLE HOT-STATE)</div>
                <div className="p-2 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] break-all text-[11px] text-[#1a7f37] dark:text-[#3fb950] flex items-center justify-between gap-2">
                  <span>{block.state_root}</span>
                  <CopyButton text={block.state_root} />
                </div>
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Card 3: Daftar Transaksi dalam Blok */}
        <Card>
          <CardHeader className="py-2.5 px-4">
            <CardTitle className="text-xs">
              <Box className="w-4 h-4 text-[#0969da] dark:text-[#58a6ff]" />
              <span>DAFTAR TRANSAKSI ({block.transactions.length} TRANSAKSI TERKONFIRMASI)</span>
            </CardTitle>
          </CardHeader>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>TRANSACTION HASH</TableHead>
                  <TableHead>PENGIRIM (FROM)</TableHead>
                  <TableHead>PENERIMA (TO)</TableHead>
                  <TableHead>NOMINAL (AUR)</TableHead>
                  <TableHead>FEE</TableHead>
                  <TableHead>STATUS</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {block.transactions.map((tx) => (
                  <TableRow key={tx.hash}>
                    <TableCell className="font-semibold whitespace-nowrap">
                      <Link
                        href={`/tx/${tx.hash}`}
                        className="text-[#0969da] dark:text-[#58a6ff] hover:underline"
                      >
                        {tx.hash.slice(0, 14)}...
                      </Link>
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      <Link
                        href={`/account/${tx.sender}`}
                        className="text-[#656d76] dark:text-[#8b949e] hover:text-[#0969da] dark:hover:text-[#58a6ff] hover:underline"
                      >
                        {tx.sender.slice(0, 12)}...
                      </Link>
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      <Link
                        href={`/account/${tx.receiver}`}
                        className="text-[#656d76] dark:text-[#8b949e] hover:text-[#0969da] dark:hover:text-[#58a6ff] hover:underline"
                      >
                        {tx.receiver.slice(0, 12)}...
                      </Link>
                    </TableCell>
                    <TableCell className="font-bold text-[#1a7f37] dark:text-[#3fb950] whitespace-nowrap">
                      {tx.amount_aur}
                    </TableCell>
                    <TableCell className="text-[#656d76] dark:text-[#8b949e] whitespace-nowrap">
                      {tx.network_fee_aur}
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      <Badge variant="success" className="py-0 text-[10px]">
                        Finalized
                      </Badge>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </Card>
      </main>
    </div>
  );
}
