import React from "react";
import {
  Globe,
  CheckCircle2,
  ExternalLink,
  ShieldCheck,
  Cpu,
  Layers,
  Clock,
  ArrowRight,
  Copy,
  Check,
} from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "./ui/dialog";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";

interface ExplorerModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  txHash?: string | null;
}

export const ExplorerModal: React.FC<ExplorerModalProps> = ({
  open,
  onOpenChange,
  txHash,
}) => {
  const [copied, setCopied] = React.useState(false);

  const hash = txHash || "0x7a9cf1832b904d805ea2cf847bc01309f927e28a55cc8294101e40a4daec91b2";

  const handleCopy = () => {
    navigator.clipboard.writeText(hash);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent onClose={() => onOpenChange(false)} className="max-w-xl">
        <DialogHeader>
          <div className="flex items-center gap-2 mb-1">
            <div className="p-1.5 rounded-lg bg-cyan-950/60 border border-cyan-500/30 text-cyan-400">
              <Globe className="h-4 w-4" />
            </div>
            <DialogTitle>Aurion Testnet Explorer</DialogTitle>
          </div>
          <DialogDescription>
            Detail transaksi dan verifikasi blok terdesentralisasi pada Chain ID 1001.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 text-xs font-mono">
          {/* Status Bar */}
          <div className="flex items-center justify-between p-3 rounded-xl bg-emerald-950/30 border border-emerald-500/30 text-emerald-300">
            <div className="flex items-center gap-2">
              <CheckCircle2 className="h-4 w-4 text-emerald-400" />
              <span className="font-sans font-bold">Status Transaksi: Sukses</span>
            </div>
            <Badge variant="emerald" className="font-mono text-[10px]">
              12 Konfirmasi Blok
            </Badge>
          </div>

          {/* Hash view */}
          <div className="p-3.5 rounded-xl bg-zinc-950 border border-zinc-800 space-y-1.5">
            <div className="flex items-center justify-between text-zinc-400 font-sans text-[11px]">
              <span>Blake3 Transaction Hash</span>
              <button
                type="button"
                onClick={handleCopy}
                className="text-cyan-400 hover:text-cyan-300 flex items-center gap-1 cursor-pointer"
              >
                {copied ? <Check className="h-3 w-3 text-emerald-400" /> : <Copy className="h-3 w-3" />}
                <span>{copied ? "Tersalin" : "Salin"}</span>
              </button>
            </div>
            <p className="text-cyan-300 break-all select-all font-mono text-[11px] leading-relaxed">
              {hash}
            </p>
          </div>

          {/* Details table */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-950 divide-y divide-zinc-800/80">
            <div className="p-2.5 flex items-center justify-between">
              <span className="text-zinc-500 font-sans">Block Height:</span>
              <span className="text-white font-bold">#1,482,918</span>
            </div>
            <div className="p-2.5 flex items-center justify-between">
              <span className="text-zinc-500 font-sans">Timestamp:</span>
              <span className="text-zinc-300 font-sans">Baru saja (Finalized)</span>
            </div>
            <div className="p-2.5 flex items-center justify-between">
              <span className="text-zinc-500 font-sans">Dari (Genesis Reserve):</span>
              <span className="text-zinc-400 truncate max-w-[240px]">0x000000000000000000000000000000000000A001</span>
            </div>
            <div className="p-2.5 flex items-center justify-between">
              <span className="text-zinc-500 font-sans">Jumlah Dikirim:</span>
              <span className="text-emerald-400 font-bold font-mono">10.000000 AUR</span>
            </div>
            <div className="p-2.5 flex items-center justify-between">
              <span className="text-zinc-500 font-sans">Biaya Transaksi (Gas):</span>
              <span className="text-zinc-400 font-mono">0.00021 AUR (21 Gwei)</span>
            </div>
          </div>
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => onOpenChange(false)}
            className="text-xs border-zinc-800 text-zinc-300"
          >
            Tutup
          </Button>
          <Button
            variant="default"
            size="sm"
            onClick={() => {
              window.open(`https://explorer.testnet.aurion.network/tx/${hash}`, "_blank");
            }}
            className="text-xs font-semibold gap-1.5"
          >
            <span>Buka di AurionScan</span>
            <ExternalLink className="h-3 w-3" />
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
};
