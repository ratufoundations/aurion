import React, { useState } from "react";
import {
  Layers,
  Clock,
  PlusCircle,
  ChevronDown,
  Copy,
  Check,
  Zap,
  Shield,
  ExternalLink,
  Wallet,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "./ui/card";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";

interface InfoSectionProps {
  onAddNetworkToWallet?: () => void;
}

export const InfoSection: React.FC<InfoSectionProps> = ({ onAddNetworkToWallet }) => {
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const copyToClipboard = (text: string, key: string) => {
    navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 2000);
  };

  return (
    <div className="w-full max-w-4xl mx-auto mt-12 space-y-4">
      <div className="flex items-center justify-between px-2">
        <h2 className="text-lg font-bold text-white tracking-tight flex items-center gap-2">
          <Zap className="h-4 w-4 text-cyan-400" />
          <span>Panduan & Informasi Jaringan</span>
        </h2>
        <span className="text-xs text-zinc-500 hidden sm:inline">
          Aurion Testnet Specifications
        </span>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Card 1: Apa itu Aurion Testnet? */}
        <Card className="border-zinc-800/80 bg-zinc-900/40 hover:border-cyan-500/40 hover:bg-zinc-900/60 transition-all duration-300">
          <CardHeader className="p-5 pb-3">
            <div className="flex items-center gap-2.5 mb-2">
              <div className="p-2 rounded-xl bg-cyan-950/60 border border-cyan-500/30 text-cyan-400">
                <Layers className="h-4 w-4" />
              </div>
              <Badge variant="cyan" className="text-[10px]">
                Layer-1 Network
              </Badge>
            </div>
            <CardTitle className="text-base font-bold text-white">
              Apa itu Aurion Testnet?
            </CardTitle>
          </CardHeader>
          <CardContent className="p-5 pt-0 text-xs text-zinc-400 leading-relaxed space-y-2.5">
            <p>
              Aurion Testnet adalah lingkungan sandbox Layer-1 berkinerja tinggi berbasis konsensus Proof-of-Stake yang dirancang untuk pengujian smart contract EVM & WASM.
            </p>
            <div className="pt-2 border-t border-zinc-800/80 flex items-center justify-between text-[11px] text-zinc-500 font-mono">
              <span>Finalitas Blok: ~1.2 detik</span>
              <span className="text-cyan-400">Blake3 Hash</span>
            </div>
          </CardContent>
        </Card>

        {/* Card 2: Aturan Cooldown 24 Jam */}
        <Card className="border-zinc-800/80 bg-zinc-900/40 hover:border-emerald-500/40 hover:bg-zinc-900/60 transition-all duration-300">
          <CardHeader className="p-5 pb-3">
            <div className="flex items-center gap-2.5 mb-2">
              <div className="p-2 rounded-xl bg-emerald-950/60 border border-emerald-500/30 text-emerald-400">
                <Clock className="h-4 w-4" />
              </div>
              <Badge variant="emerald" className="text-[10px]">
                Anti-Sybil Policy
              </Badge>
            </div>
            <CardTitle className="text-base font-bold text-white">
              Aturan Cooldown 24 Jam
            </CardTitle>
          </CardHeader>
          <CardContent className="p-5 pt-0 text-xs text-zinc-400 leading-relaxed space-y-2.5">
            <p>
              Untuk menjaga ketersediaan likuiditas bagi seluruh pengembang, sistem membatasi permintaan maksimal <strong>10 AUR per address & IP</strong> setiap interval 24 jam.
            </p>
            <div className="pt-2 border-t border-zinc-800/80 flex items-center justify-between text-[11px] text-zinc-500 font-mono">
              <span>Rate Limit: 1 req/hari</span>
              <span className="text-emerald-400">Genesis Pool</span>
            </div>
          </CardContent>
        </Card>

        {/* Card 3: Cara Menambahkan Jaringan ke Dompet */}
        <Card className="border-zinc-800/80 bg-zinc-900/40 hover:border-indigo-500/40 hover:bg-zinc-900/60 transition-all duration-300 flex flex-col justify-between">
          <CardHeader className="p-5 pb-3">
            <div className="flex items-center gap-2.5 mb-2">
              <div className="p-2 rounded-xl bg-indigo-950/60 border border-indigo-500/30 text-indigo-400">
                <PlusCircle className="h-4 w-4" />
              </div>
              <Badge variant="default" className="text-[10px] bg-indigo-950 text-indigo-300 border-indigo-500/30">
                RPC Setup
              </Badge>
            </div>
            <CardTitle className="text-base font-bold text-white">
              Cara Menambahkan ke Dompet
            </CardTitle>
          </CardHeader>
          <CardContent className="p-5 pt-0 text-xs text-zinc-400 leading-relaxed space-y-3">
            <p>
              Konfigurasikan MetaMask, Rabby, atau dompet Web3 Anda dengan parameter resmi:
            </p>

            <div className="space-y-1.5 font-mono text-[11px] bg-zinc-950/80 p-2.5 rounded-xl border border-zinc-800/80">
              <div className="flex items-center justify-between">
                <span className="text-zinc-500">Chain ID:</span>
                <span className="text-cyan-300 font-bold">1001 (0x3E9)</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-zinc-500">Simbol:</span>
                <span className="text-zinc-200 font-bold">AUR</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-zinc-500 truncate mr-1">RPC:</span>
                <button
                  type="button"
                  onClick={() => copyToClipboard("https://rpc.testnet.aurion.network", "rpc")}
                  className="text-cyan-400 hover:underline flex items-center gap-1 cursor-pointer"
                >
                  <span>rpc.testnet.aurion.network</span>
                  {copiedKey === "rpc" ? <Check className="h-3 w-3 text-emerald-400" /> : <Copy className="h-3 w-3" />}
                </button>
              </div>
            </div>

            {onAddNetworkToWallet && (
              <Button
                variant="subtle"
                size="sm"
                onClick={onAddNetworkToWallet}
                className="w-full h-8 text-xs font-semibold gap-1.5"
              >
                <Wallet className="h-3.5 w-3.5" />
                <span>Tambahkan Otomatis ke Dompet</span>
              </Button>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
};
