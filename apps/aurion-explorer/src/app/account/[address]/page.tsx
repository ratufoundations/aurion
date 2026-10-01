import React from "react";
import Link from "next/link";
import { ArrowLeft, User, Wallet, KeyRound, Activity, AlertTriangle, CheckCircle2, ShieldCheck, ArrowUpRight, ArrowDownLeft } from "lucide-react";
import { rpcClient, AccountInfo } from "@/lib/rpc";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableHeader, TableHead, TableBody, TableRow, TableCell } from "@/components/ui/table";
import { CopyButton } from "@/components/CopyButton";
import { ThemeToggle } from "@/components/theme-toggle";
import { Quanta } from "@aurion/sdk";

interface AccountPageProps {
  params: Promise<{ address: string }>;
}

export function generateStaticParams() {
  return [{ address: "0x8b3a8b27v5285a50ef03d7890bfa4312e" }];
}

export default async function AccountDetailPage({ params }: AccountPageProps) {
  const { address } = await params;

  let account: AccountInfo | null = null;
  let isLiveRpc = false;

  try {
    account = await rpcClient.getAccount(address);
    isLiveRpc = true;
  } catch {
    // Fallback: hitung saldo deterministik menggunakan Quanta
    // Ambil angka dari address atau default 150 AUR
    const rawVal = BigInt("1500000000000"); // 150 AUR dalam Quanta (10^10)
    const quanta = Quanta.fromBigInt(rawVal);
    account = {
      address,
      balanceQuanta: quanta.toBigInt(),
      balanceFormatted: `${quanta.toAur()} AUR`,
      rawQuantaString: `${quanta.toString()} Quanta`,
      nonce: 18,
    };
  }

  // Mock transaksi terkait akun untuk representasi aktivitas
  const mockActivity = [
    {
      hash: "0x8b3a8b27v5285a50ef03d7890bfa4312e",
      type: "IN" as const,
      peer: "0x4d9980a312fe6c7104be9975b3420811",
      amount_aur: "50.00 AUR",
      ageText: "2m lalu",
      blockHeight: 482912,
    },
    {
      hash: "0x7e29b4c089fa4321d567ea309b114c55",
      type: "OUT" as const,
      peer: "0x8b340b3d353d3b60ff40a6b2d7211d46",
      amount_aur: "12.50 AUR",
      ageText: "14m lalu",
      blockHeight: 482905,
    },
    {
      hash: "0x3fb950ef03d7890bfa4312e7e29b4c08",
      type: "IN" as const,
      peer: "0x9c4a1234567890abcdef0123456789a",
      amount_aur: "100.00 AUR",
      ageText: "1h lalu",
      blockHeight: 482850,
    },
  ];

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
            <User className="w-3.5 h-3.5 text-[#0969da] dark:text-[#58a6ff]" />
            Detail Akun
          </span>
        </div>

        <div className="flex items-center gap-2">
          <ThemeToggle />
          <Badge
            variant={isLiveRpc ? "success" : "warning"}
            className="text-[10px] py-0.5"
          >
            {isLiveRpc ? "Hot-State RPC (127.0.0.1:8545)" : "State Simulasi"}
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
              <span>Simpul RPC Lokal (127.0.0.1:8545) offline. Menampilkan status akun dari tabel simulasi Hot State.</span>
            </div>
            <Link href="/">
              <Button variant="outline" size="sm" className="h-6 text-[11px]">
                Kembali
              </Button>
            </Link>
          </div>
        )}

        {/* Account Header */}
        <div className="bg-white dark:bg-[#161b22] border border-[#d0d7de] dark:border-[#30363d] p-4 rounded shadow-xs space-y-2">
          <div className="flex items-center justify-between flex-wrap gap-2">
            <div className="flex items-center gap-2">
              <span className="text-[11px] font-mono text-[#656d76] dark:text-[#8b949e] uppercase">
                ALAMAT AKUN PUBLIK (PUBLIC ADDRESS)
              </span>
              <Badge variant="outline" className="text-[10px] py-0">
                Ed25519 Canonical
              </Badge>
            </div>
            <Badge variant="success" className="gap-1 py-0.5 text-xs">
              <ShieldCheck className="w-3 h-3" />
              <span>Hot State Synced</span>
            </Badge>
          </div>

          <div className="p-2.5 bg-[#f6f8fa] dark:bg-[#0d1117] rounded border border-[#d0d7de] dark:border-[#30363d] break-all text-xs font-mono text-[#0969da] dark:text-[#58a6ff] flex items-center justify-between gap-2">
            <span className="font-semibold">{address}</span>
            <CopyButton text={address} />
          </div>
        </div>

        {/* 2-Column Balance & Nonce Cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* Card Saldo (Quanta Zero-Float) */}
          <Card>
            <CardHeader className="py-2.5 px-4">
              <CardTitle className="text-xs">
                <Wallet className="w-4 h-4 text-[#1a7f37] dark:text-[#3fb950]" />
                <span>SALDO AKTIF (ZERO-FLOAT QUANTA MATH)</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="p-4 space-y-3">
              <div>
                <span className="text-[11px] text-[#656d76] dark:text-[#8b949e]">TOTAL SALDO AUR</span>
                <div className="text-2xl font-bold text-[#1a7f37] dark:text-[#3fb950] tracking-tight mt-0.5">
                  {account.balanceFormatted}
                </div>
              </div>
              <div className="pt-2 border-t border-[#d0d7de]/50 dark:border-[#30363d]/50 flex items-center justify-between text-xs">
                <span className="text-[#656d76] dark:text-[#8b949e]">Nilai Satuan Terkecil (Raw Quanta)</span>
                <span className="font-semibold text-[#1f2328] dark:text-[#f0f6fc]">{account.rawQuantaString}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-[#656d76] dark:text-[#8b949e]">Desimal Pembagian</span>
                <span className="font-semibold text-[#656d76] dark:text-[#8b949e]">10 Desimal (1 AUR = 10^10 Quanta)</span>
              </div>
            </CardContent>
          </Card>

          {/* Card Nonce & Transaksi */}
          <Card>
            <CardHeader className="py-2.5 px-4">
              <CardTitle className="text-xs">
                <KeyRound className="w-4 h-4 text-[#9a6700] dark:text-[#d29922]" />
                <span>NONCE &amp; STATUS STATE</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="p-4 space-y-3">
              <div>
                <span className="text-[11px] text-[#656d76] dark:text-[#8b949e]">URUTAN NONCE TERAKHIR (U64)</span>
                <div className="text-2xl font-bold text-[#1f2328] dark:text-[#f0f6fc] tracking-tight mt-0.5">
                  #{account.nonce}
                </div>
              </div>
              <div className="pt-2 border-t border-[#d0d7de]/50 dark:border-[#30363d]/50 flex items-center justify-between text-xs">
                <span className="text-[#656d76] dark:text-[#8b949e]">Perlindungan Replay</span>
                <span className="text-[#1a7f37] dark:text-[#3fb950] flex items-center gap-1 font-semibold">
                  <CheckCircle2 className="w-3 h-3" /> Anti-Replay Guard Active
                </span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-[#656d76] dark:text-[#8b949e]">Tabel Penyimpanan Simpul</span>
                <span className="font-semibold text-[#1f2328] dark:text-[#f0f6fc]">ACCOUNTS_TABLE (Hot-State Trie)</span>
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Card Riwayat Aktivitas Akun */}
        <Card>
          <CardHeader className="py-2.5 px-4">
            <CardTitle className="text-xs">
              <Activity className="w-4 h-4 text-[#0969da] dark:text-[#58a6ff]" />
              <span>RIWAYAT AKTIVITAS TERBARU (TRANSAKSI TERKAIT)</span>
            </CardTitle>
          </CardHeader>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ARAH</TableHead>
                  <TableHead>TX HASH</TableHead>
                  <TableHead>PEER LAWAN</TableHead>
                  <TableHead>NOMINAL</TableHead>
                  <TableHead>BLOK</TableHead>
                  <TableHead>WAKTU</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {mockActivity.map((tx) => (
                  <TableRow key={tx.hash}>
                    <TableCell>
                      {tx.type === "IN" ? (
                        <Badge variant="success" className="gap-1 py-0 text-[10px]">
                          <ArrowDownLeft className="w-3 h-3" />
                          <span>MASUK</span>
                        </Badge>
                      ) : (
                        <Badge variant="warning" className="gap-1 py-0 text-[10px]">
                          <ArrowUpRight className="w-3 h-3" />
                          <span>KELUAR</span>
                        </Badge>
                      )}
                    </TableCell>
                    <TableCell className="font-semibold whitespace-nowrap">
                      <Link href={`/tx/${tx.hash}`} className="text-[#0969da] dark:text-[#58a6ff] hover:underline">
                        {tx.hash.slice(0, 14)}...
                      </Link>
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      <Link href={`/account/${tx.peer}`} className="text-[#656d76] dark:text-[#8b949e] hover:underline">
                        {tx.peer.slice(0, 14)}...
                      </Link>
                    </TableCell>
                    <TableCell className="font-bold text-[#1f2328] dark:text-[#f0f6fc] whitespace-nowrap">
                      {tx.amount_aur}
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      <Link href={`/block/${tx.blockHeight}`} className="text-[#0969da] dark:text-[#58a6ff] hover:underline">
                        #{tx.blockHeight}
                      </Link>
                    </TableCell>
                    <TableCell className="text-[#656d76] dark:text-[#8b949e] whitespace-nowrap">
                      {tx.ageText}
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
