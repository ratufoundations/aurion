import React, { useState } from "react";
import {
  CheckCircle2,
  Copy,
  Check,
  ExternalLink,
  Coins,
  Layers,
  ArrowUpRight,
  ShieldCheck,
  RefreshCw,
} from "lucide-react";
import { ClaimSuccessResponse } from "../types";
import { Card, CardContent } from "./ui/card";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { WalletQrCard } from "./WalletQrCard";

interface SuccessCardProps {
  data: ClaimSuccessResponse;
  onDismiss: () => void;
  onOpenExplorer: (txHash: string) => void;
}

export const SuccessCard: React.FC<SuccessCardProps> = ({
  data,
  onDismiss,
  onOpenExplorer,
}) => {
  const [copied, setCopied] = useState(false);

  const formatHashShort = (hash: string) => {
    if (!hash || hash.length < 18) return hash;
    return `${hash.slice(0, 10)}...${hash.slice(-8)}`;
  };

  const handleCopyHash = async () => {
    try {
      await navigator.clipboard.writeText(data.tx_hash);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      // Fallback
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="relative w-full max-w-2xl mx-auto animate-in fade-in zoom-in-95 duration-300">
      {/* Glow highlight behind success card */}
      <div className="absolute -inset-1 rounded-3xl bg-gradient-to-r from-emerald-500/30 via-cyan-500/20 to-emerald-500/30 blur-xl opacity-80" />

      <Card className="relative border-emerald-500/40 bg-zinc-950/95 backdrop-blur-2xl shadow-2xl shadow-emerald-950/20 overflow-hidden">
        {/* Top accent line */}
        <div className="h-1.5 w-full bg-gradient-to-r from-emerald-400 via-cyan-400 to-emerald-500" />

        <CardContent className="p-6 sm:p-8 space-y-6">
          {/* Header with glowing icon */}
          <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
            <div className="flex items-center gap-3.5">
              <div className="relative flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 shadow-lg shadow-emerald-500/20">
                <CheckCircle2 className="h-7 w-7 text-emerald-400 animate-bounce-once" />
                <span className="absolute -top-1 -right-1 flex h-3 w-3">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75" />
                  <span className="relative inline-flex rounded-full h-3 w-3 bg-emerald-500" />
                </span>
              </div>
              <div>
                <div className="flex items-center gap-2">
                  <h3 className="text-xl font-bold text-white tracking-tight">
                    Klaim Faucet Berhasil!
                  </h3>
                  <Badge variant="emerald" className="text-[10px] uppercase tracking-wider">
                    Success 200 OK
                  </Badge>
                </div>
                <p className="text-xs sm:text-sm text-zinc-400 mt-0.5">
                  10 AUR telah dikirimkan ke wallet Anda melalui Genesis Reserve.
                </p>
              </div>
            </div>

            {/* Quick action to claim another or dismiss */}
            <Button
              variant="outline"
              size="sm"
              onClick={onDismiss}
              className="text-xs h-8 text-zinc-400 hover:text-white border-zinc-800"
            >
              <RefreshCw className="h-3 w-3 mr-1.5" />
              Klaim Lainnya
            </Button>
          </div>

          {/* Transaction Hash Canonical Box */}
          <div className="rounded-2xl border border-zinc-800/80 bg-zinc-900/60 p-4 space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-zinc-400 uppercase tracking-wider flex items-center gap-1.5">
                <ShieldCheck className="h-3.5 w-3.5 text-cyan-400" />
                Canonical Transaction Hash (Blake3)
              </span>
              <span className="text-[11px] text-emerald-400 font-mono font-medium">
                Confirmed in Block #{data.block_height.toLocaleString()}
              </span>
            </div>

            <div className="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-2.5 pt-1">
              <div className="flex-1 font-mono text-xs sm:text-sm text-cyan-300 bg-zinc-950/80 px-3.5 py-2.5 rounded-xl border border-zinc-800/80 break-all select-all">
                <span className="hidden sm:inline" title={data.tx_hash}>
                  {data.tx_hash}
                </span>
                <span className="sm:hidden">
                  {formatHashShort(data.tx_hash)}
                </span>
              </div>

              <div className="flex items-center gap-2">
                <Button
                  id="copy-tx-hash-btn"
                  variant="secondary"
                  size="sm"
                  onClick={handleCopyHash}
                  className="h-10 px-3.5 text-xs bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border-zinc-700 flex-1 sm:flex-initial"
                >
                  {copied ? (
                    <>
                      <Check className="h-3.5 w-3.5 text-emerald-400 mr-1.5" />
                      <span className="text-emerald-400 font-semibold">Tersalin!</span>
                    </>
                  ) : (
                    <>
                      <Copy className="h-3.5 w-3.5 text-zinc-400 mr-1.5" />
                      <span>Copy Tx Hash</span>
                    </>
                  )}
                </Button>

                <Button
                  id="view-explorer-btn"
                  variant="subtle"
                  size="sm"
                  onClick={() => onOpenExplorer(data.tx_hash)}
                  className="h-10 px-3.5 text-xs flex-1 sm:flex-initial"
                >
                  <ExternalLink className="h-3.5 w-3.5 mr-1.5" />
                  <span>View in Explorer</span>
                </Button>
              </div>
            </div>
          </div>

          {/* Meta Details Grid */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5 pt-1">
            <div className="rounded-xl bg-zinc-900/40 border border-zinc-800/60 p-3">
              <span className="text-[10px] uppercase font-semibold text-zinc-400 block mb-1">
                Jumlah Transfer
              </span>
              <div className="flex items-baseline gap-1">
                <span className="text-base font-extrabold text-white font-mono">
                  +{data.amount}
                </span>
                <span className="text-xs font-bold text-cyan-400">{data.symbol}</span>
              </div>
            </div>

            <div className="rounded-xl bg-zinc-900/40 border border-zinc-800/60 p-3">
              <span className="text-[10px] uppercase font-semibold text-zinc-400 block mb-1">
                Biaya Gas (Gas Fee)
              </span>
              <span className="text-xs font-mono font-medium text-emerald-400 block">
                {data.gas_fee}
              </span>
            </div>

            <div className="rounded-xl bg-zinc-900/40 border border-zinc-800/60 p-3">
              <span className="text-[10px] uppercase font-semibold text-zinc-400 block mb-1">
                Block Height
              </span>
              <span className="text-xs font-mono font-bold text-zinc-200 block">
                #{data.block_height.toLocaleString()}
              </span>
            </div>

            <div className="rounded-xl bg-zinc-900/40 border border-zinc-800/60 p-3">
              <span className="text-[10px] uppercase font-semibold text-zinc-400 block mb-1">
                Jaringan
              </span>
              <span className="text-xs font-semibold text-zinc-300 block truncate">
                Aurion Testnet
              </span>
            </div>
          </div>

          {/* QR Code for User Wallet Address (Receive Funds / Share) */}
          <WalletQrCard
            address={data.recipient}
            symbol={data.symbol || "AUR"}
            chainId={1001}
            networkName="Aurion Testnet"
          />

          {/* Address target review footer */}
          <div className="text-xs text-zinc-400 bg-zinc-900/30 rounded-xl p-3 border border-zinc-800/40 flex flex-col sm:flex-row sm:items-center justify-between gap-2">
            <span className="text-zinc-500">Alamat Penerima Resmi:</span>
            <span className="font-mono text-zinc-300 break-all select-all font-medium">
              {data.recipient}
            </span>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};
