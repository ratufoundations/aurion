import React from "react";
import { BookOpen, Code2, Terminal, ExternalLink, ShieldCheck, Cpu } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "./ui/dialog";
import { Button } from "./ui/button";

interface DocsModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export const DocsModal: React.FC<DocsModalProps> = ({ open, onOpenChange }) => {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent onClose={() => onOpenChange(false)} className="max-w-xl">
        <DialogHeader>
          <div className="flex items-center gap-2 mb-1">
            <div className="p-1.5 rounded-lg bg-emerald-950/60 border border-emerald-500/30 text-emerald-400">
              <BookOpen className="h-4 w-4" />
            </div>
            <DialogTitle>Dokumentasi Aurion Faucet API</DialogTitle>
          </div>
          <DialogDescription>
            Panduan integrasi programmatic faucet untuk pipeline CI/CD dan pengujian smart contract.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 text-xs">
          <div>
            <span className="font-bold text-zinc-200 block mb-1">
              1. HTTP Endpoint (cURL Example)
            </span>
            <div className="bg-zinc-950 p-3 rounded-xl border border-zinc-800 font-mono text-[11px] text-cyan-300 overflow-x-auto">
              curl -X POST https://faucet.testnet.aurion.network/api/v1/claim \<br />
              &nbsp;&nbsp;-H "Content-Type: application/json" \<br />
              &nbsp;&nbsp;-d '&#123; "target_address": "0x71C27aA5208b53C0D0f845A95B29B87F7D032849" &#125;'
            </div>
          </div>

          <div>
            <span className="font-bold text-zinc-200 block mb-1">
              2. Kode Status Respon
            </span>
            <ul className="space-y-1.5 text-zinc-400">
              <li className="flex items-center gap-2">
                <span className="px-1.5 py-0.5 rounded bg-emerald-950 border border-emerald-500/30 text-emerald-400 font-mono text-[10px]">
                  200 OK
                </span>
                <span>Token berhasil dikirimkan, mengembalikan Blake3 Tx Hash.</span>
              </li>
              <li className="flex items-center gap-2">
                <span className="px-1.5 py-0.5 rounded bg-amber-950 border border-amber-500/30 text-amber-400 font-mono text-[10px]">
                  429 Too Many Requests
                </span>
                <span>Alamat atau IP berada dalam masa cooldown 24 jam.</span>
              </li>
              <li className="flex items-center gap-2">
                <span className="px-1.5 py-0.5 rounded bg-red-950 border border-red-500/30 text-red-400 font-mono text-[10px]">
                  503 Service Unavailable
                </span>
                <span>Koneksi bootnode sinkronisasi atau beban tinggi sementara.</span>
              </li>
            </ul>
          </div>
        </div>

        <div className="mt-5 flex justify-end">
          <Button
            variant="default"
            size="sm"
            onClick={() => onOpenChange(false)}
            className="text-xs"
          >
            Tutup
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
};
