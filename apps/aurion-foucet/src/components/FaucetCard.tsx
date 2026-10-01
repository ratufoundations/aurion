import React, { useState } from "react";
import {
  Wallet,
  Clipboard,
  X,
  Sparkles,
  Loader2,
  CheckCircle2,
  AlertCircle,
  Coins,
  Clock,
  Database,
  ArrowRight,
  QrCode,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Badge } from "./ui/badge";
import { Card, CardContent } from "./ui/card";
import { QrScannerOverlay } from "./QrScannerOverlay";
import { cn } from "../lib/utils";

interface FaucetCardProps {
  address: string;
  setAddress: (val: string) => void;
  isLoading: boolean;
  onClaim: () => void;
  isCooldownActive: boolean;
  cooldownRemainingSeconds?: number;
  reserveBalance?: number;
}

export const FaucetCard: React.FC<FaucetCardProps> = ({
  address,
  setAddress,
  isLoading,
  onClaim,
  isCooldownActive,
  cooldownRemainingSeconds = 0,
  reserveBalance = 4850210,
}) => {
  const [pasteError, setPasteError] = useState<string | null>(null);
  const [isScannerOpen, setIsScannerOpen] = useState<boolean>(false);

  // Address validation checks
  const trimmed = address.trim();
  const hasPrefix = trimmed.startsWith("0x");
  const isValidEvm = /^0x[a-fA-F0-9]{40}$/.test(trimmed);
  const isValidBlake3 = /^0x[a-fA-F0-9]{64}$/.test(trimmed);
  const isValidAddress = isValidEvm || isValidBlake3;
  const isNotEmpty = trimmed.length > 0;
  const isButtonEnabled = isValidAddress && !isLoading && !isCooldownActive;

  const handleScanSuccess = (scannedAddress: string) => {
    setAddress(scannedAddress);
    toast.success("Alamat wallet berhasil dipindai dari QR code!", {
      description: `${scannedAddress.slice(0, 10)}...${scannedAddress.slice(-8)} siap untuk klaim token AUR`,
    });
  };

  const handlePaste = async () => {
    setPasteError(null);
    try {
      if (navigator.clipboard && navigator.clipboard.readText) {
        const text = await navigator.clipboard.readText();
        if (text) {
          setAddress(text.trim());
        }
      } else {
        const manual = prompt("Tempel (Paste) alamat wallet Aurion Anda di sini:");
        if (manual) setAddress(manual.trim());
      }
    } catch (err) {
      const manual = prompt("Akses clipboard ditolak. Silakan tempel alamat secara manual:");
      if (manual) setAddress(manual.trim());
    }
  };

  const handleClear = () => {
    setAddress("");
    setPasteError(null);
  };

  const handleUseSampleAddress = () => {
    setAddress("0x71C27aA5208b53C0D0f845A95B29B87F7D032849");
  };

  const formatTimeRemaining = (seconds: number) => {
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${hrs.toString().padStart(2, "0")}:${mins
      .toString()
      .padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  };

  return (
    <div className="relative w-full max-w-2xl mx-auto">
      {/* Subtle background ambient glow */}
      <div className="absolute -inset-1 rounded-3xl bg-gradient-to-r from-cyan-500/20 via-emerald-500/20 to-indigo-500/20 blur-xl opacity-75 transition-opacity" />

      <Card className="relative border-zinc-800/90 bg-zinc-950/90 backdrop-blur-2xl shadow-2xl shadow-cyan-950/10">
        <CardContent className="p-6 sm:p-8">
          {/* Header section with badge */}
          <div className="text-center mb-6 sm:mb-8">
            <div className="inline-flex items-center gap-1.5 rounded-full border border-cyan-500/30 bg-cyan-950/30 px-3 py-1 text-xs font-semibold text-cyan-300 mb-3">
              <Sparkles className="h-3.5 w-3.5 text-cyan-400" />
              <span>Official Aurion Testnet Faucet</span>
            </div>
            <h1 className="text-2xl sm:text-3xl lg:text-4xl font-extrabold tracking-tight text-white mb-2 font-mono">
              Aurion Testnet Faucet
            </h1>
            <p className="text-sm sm:text-base text-zinc-400 max-w-lg mx-auto leading-relaxed">
              Dapatkan saldo uji coba AUR untuk kebutuhan deployment dan pengujian transaksi di jaringan testnet.
            </p>
          </div>

          {/* Form Area */}
          <div className="space-y-4">
            {/* Input Label & Quick Helper */}
            <div className="flex items-center justify-between text-xs">
              <label htmlFor="aurion-address-input" className="font-semibold text-zinc-300 flex items-center gap-1.5">
                <span>Alamat Wallet Penerima (Recipient Address)</span>
              </label>
              <div className="flex items-center gap-3">
                <button
                  type="button"
                  onClick={() => setIsScannerOpen(true)}
                  className="inline-flex items-center gap-1 text-cyan-400 hover:text-cyan-300 transition-colors font-mono cursor-pointer hover:underline"
                  title="Pindai QR code alamat wallet dari kamera"
                >
                  <QrCode className="h-3 w-3" />
                  <span>Scan QR</span>
                </button>
                <button
                  type="button"
                  onClick={handleUseSampleAddress}
                  className="text-zinc-400 hover:text-zinc-300 transition-colors font-mono cursor-pointer hover:underline"
                >
                  Contoh Address
                </button>
              </div>
            </div>

            {/* Input Field with Prefix & Inline Buttons */}
            <div className="relative flex items-center">
              <div className="pointer-events-none absolute left-3.5 flex items-center text-zinc-500">
                <Wallet className="h-5 w-5 text-cyan-400/80" />
              </div>

              <Input
                id="aurion-address-input"
                type="text"
                placeholder="0x... (Alamat hex 40 atau 64 karakter)"
                value={address}
                onChange={(e) => setAddress(e.target.value)}
                disabled={isLoading}
                className="h-14 pl-11 pr-44 sm:pr-48 font-mono text-xs sm:text-sm bg-zinc-900/90 border-zinc-800 text-zinc-100 placeholder:text-zinc-600 focus-visible:border-cyan-500 focus-visible:ring-cyan-500/20"
              />

              {/* Action buttons inside input */}
              <div className="absolute right-2 flex items-center gap-1">
                {isNotEmpty && (
                  <button
                    type="button"
                    onClick={handleClear}
                    title="Hapus address"
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-zinc-800 transition-colors"
                  >
                    <X className="h-4 w-4" />
                  </button>
                )}
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  onClick={handlePaste}
                  title="Tempel dari Clipboard"
                  className="h-8 px-2 text-xs bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border-zinc-700 flex items-center gap-1"
                >
                  <Clipboard className="h-3 w-3 text-cyan-400" />
                  <span className="hidden sm:inline">Paste</span>
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  onClick={() => setIsScannerOpen(true)}
                  title="Pindai QR code wallet menggunakan kamera"
                  className="h-8 px-2.5 text-xs bg-cyan-950/70 hover:bg-cyan-900/90 text-cyan-300 border-cyan-800/80 hover:border-cyan-500/60 flex items-center gap-1.5 shadow-sm transition-all"
                >
                  <QrCode className="h-3.5 w-3.5 text-cyan-400" />
                  <span>Scan</span>
                </Button>
              </div>
            </div>

            {/* Real-time Address Validation Feedback */}
            <div className="min-h-5 flex items-center justify-between text-xs px-1">
              {isNotEmpty ? (
                isValidAddress ? (
                  <span className="flex items-center gap-1 text-emerald-400 font-medium">
                    <CheckCircle2 className="h-3.5 w-3.5" />
                    Format address valid ({isValidBlake3 ? "Aurion Blake3 Hex" : "Standard EVM Compatible"})
                  </span>
                ) : (
                  <span className="flex items-center gap-1 text-amber-400">
                    <AlertCircle className="h-3.5 w-3.5" />
                    Format belum sesuai (harus diawali 0x dengan 40 atau 64 hex karakter)
                  </span>
                )
              ) : (
                <span className="text-zinc-500">
                  Mendukung format standard EVM (0x...) dan native Blake3 Aurion
                </span>
              )}

              {isNotEmpty && (
                <span className="font-mono text-[11px] text-zinc-500">
                  {trimmed.length} karakter
                </span>
              )}
            </div>

            {/* Claim Action Button */}
            <div className="pt-2">
              <Button
                id="claim-aur-btn"
                type="button"
                onClick={onClaim}
                disabled={!isButtonEnabled}
                variant="glow"
                size="lg"
                aria-busy={isLoading}
                className={cn(
                  "w-full h-14 text-base relative overflow-hidden group font-bold tracking-wide transition-all duration-300",
                  isButtonEnabled && "animate-subtle-pulse ring-1 ring-cyan-400/40"
                )}
              >
                <span className="flex items-center justify-center gap-2 text-zinc-950">
                  {isLoading ? (
                    <Loader2 className="h-5 w-5 animate-spin text-zinc-950 shrink-0" />
                  ) : isCooldownActive ? (
                    <Clock className="h-5 w-5 shrink-0" />
                  ) : (
                    <Coins className="h-5 w-5 text-zinc-950 shrink-0" />
                  )}
                  <span>
                    {isCooldownActive
                      ? `Cooldown Aktif (${formatTimeRemaining(cooldownRemainingSeconds)})`
                      : "Request 10 AUR"}
                  </span>
                  {!isLoading && !isCooldownActive && (
                    <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-1 shrink-0" />
                  )}
                </span>
              </Button>
            </div>

            {/* Info Badges Row */}
            <div className="pt-4 border-t border-zinc-800/80 grid grid-cols-1 sm:grid-cols-3 gap-2.5">
              <div className="flex items-center gap-2.5 p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800/60">
                <div className="p-2 rounded-lg bg-cyan-950/50 text-cyan-400 border border-cyan-500/20">
                  <Coins className="h-4 w-4" />
                </div>
                <div className="flex flex-col">
                  <span className="text-[10px] uppercase font-semibold text-zinc-400">
                    Kuota Klaim
                  </span>
                  <span className="text-xs font-mono font-bold text-zinc-200">
                    10 AUR / request
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-2.5 p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800/60">
                <div className="p-2 rounded-lg bg-emerald-950/50 text-emerald-400 border border-emerald-500/20">
                  <Clock className="h-4 w-4" />
                </div>
                <div className="flex flex-col">
                  <span className="text-[10px] uppercase font-semibold text-zinc-400">
                    Batas Waktu
                  </span>
                  <span className="text-xs font-mono font-bold text-zinc-200">
                    Cooldown 24 Jam
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-2.5 p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800/60">
                <div className="p-2 rounded-lg bg-indigo-950/50 text-indigo-400 border border-indigo-500/20">
                  <Database className="h-4 w-4" />
                </div>
                <div className="flex flex-col">
                  <span className="text-[10px] uppercase font-semibold text-zinc-400">
                    Sumber Dana
                  </span>
                  <span className="text-xs font-mono font-bold text-zinc-200 truncate" title={`${reserveBalance.toLocaleString()} AUR`}>
                    Genesis Reserve Pool
                  </span>
                </div>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* QR Code Scanner Overlay Modal */}
      <QrScannerOverlay
        isOpen={isScannerOpen}
        onClose={() => setIsScannerOpen(false)}
        onScanSuccess={handleScanSuccess}
      />
    </div>
  );
};
