import React, { useEffect, useState } from "react";
import QRCode from "qrcode";
import {
  QrCode,
  Download,
  Copy,
  Check,
  Share2,
  Maximize2,
  X,
  Wallet,
  CheckCircle2,
  Sparkles,
  ArrowRight,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";

interface WalletQrCardProps {
  address: string;
  symbol?: string;
  chainId?: number;
  networkName?: string;
}

export const WalletQrCard: React.FC<WalletQrCardProps> = ({
  address,
  symbol = "AUR",
  chainId = 1001,
  networkName = "Aurion Testnet",
}) => {
  const [qrDataUrl, setQrDataUrl] = useState<string>("");
  const [copiedAddress, setCopiedAddress] = useState<boolean>(false);
  const [copiedUri, setCopiedUri] = useState<boolean>(false);
  const [isModalOpen, setIsModalOpen] = useState<boolean>(false);
  const [formatType, setFormatType] = useState<"address" | "eip681">("address");
  const [isExpanded, setIsExpanded] = useState<boolean>(true);

  // EIP-681 standard payment URI format: ethereum:0x...@1001
  const paymentUri = `ethereum:${address}@${chainId}`;
  const activeQrPayload = formatType === "address" ? address : paymentUri;

  // Generate QR Code data URL whenever activeQrPayload changes
  useEffect(() => {
    if (!address) return;

    QRCode.toDataURL(activeQrPayload, {
      width: 400,
      margin: 2,
      errorCorrectionLevel: "M",
      color: {
        dark: "#09090b", // Deep zinc black
        light: "#ffffff", // Pure white for 100% camera readability
      },
    })
      .then((url) => {
        setQrDataUrl(url);
      })
      .catch((err) => {
        console.error("Gagal membuat QR Code:", err);
      });
  }, [activeQrPayload, address]);

  // Copy full address
  const handleCopyAddress = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setCopiedAddress(true);
      toast.success("Alamat wallet berhasil disalin!", {
        description: address,
      });
      setTimeout(() => setCopiedAddress(false), 2000);
    } catch (e) {
      toast.info("Alamat tersalin!");
      setCopiedAddress(true);
      setTimeout(() => setCopiedAddress(false), 2000);
    }
  };

  // Copy standard payment URI
  const handleCopyUri = async () => {
    try {
      await navigator.clipboard.writeText(paymentUri);
      setCopiedUri(true);
      toast.success("EIP-681 Payment Link disalin!", {
        description: paymentUri,
      });
      setTimeout(() => setCopiedUri(false), 2000);
    } catch (e) {
      setCopiedUri(true);
      setTimeout(() => setCopiedUri(false), 2000);
    }
  };

  // Download QR as PNG image
  const handleDownloadQr = () => {
    if (!qrDataUrl) return;
    const link = document.createElement("a");
    const shortAddr = `${address.slice(0, 6)}...${address.slice(-4)}`;
    link.download = `aurion-qr-${shortAddr}.png`;
    link.href = qrDataUrl;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    toast.success("Gambar QR Code berhasil diunduh!", {
      description: `Disimpan sebagai aurion-qr-${shortAddr}.png`,
    });
  };

  // Web Share API
  const handleShare = async () => {
    if (typeof navigator !== "undefined" && navigator.share) {
      try {
        await navigator.share({
          title: `Alamat Wallet Aurion: ${address.slice(0, 6)}...${address.slice(-4)}`,
          text: `Kirim dana ${symbol} ke alamat wallet Aurion Testnet saya:\n${address}`,
          url: window.location.origin,
        });
        toast.success("Alamat wallet dibagikan!");
      } catch (err: any) {
        if (err.name !== "AbortError") {
          handleCopyAddress();
        }
      }
    } else {
      handleCopyAddress();
    }
  };

  return (
    <div className="rounded-2xl border border-cyan-500/30 bg-gradient-to-b from-cyan-950/20 to-zinc-950/80 p-4 sm:p-5 space-y-4">
      {/* Header bar */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <div className="p-2 rounded-xl bg-cyan-950/80 border border-cyan-500/30 text-cyan-400 shadow-sm shadow-cyan-950/50">
            <QrCode className="h-4 w-4" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h4 className="text-sm font-bold text-white tracking-tight">
                QR Code Terima Dana (Receive Funds)
              </h4>
              <Badge variant="cyan" className="text-[10px] py-0 px-1.5 font-mono">
                {symbol}
              </Badge>
            </div>
            <p className="text-xs text-zinc-400">
              Pindai dengan kamera atau mobile wallet untuk mengirim token ke alamat ini
            </p>
          </div>
        </div>

        <div className="flex items-center gap-1.5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => setIsExpanded(!isExpanded)}
            className="h-8 px-2.5 text-xs text-zinc-400 hover:text-white"
          >
            {isExpanded ? "Sembunyikan" : "Tampilkan QR"}
          </Button>
        </div>
      </div>

      {isExpanded && (
        <div className="space-y-4 pt-1 animate-in fade-in duration-200">
          {/* Main QR Display & Info Section */}
          <div className="flex flex-col sm:flex-row items-center sm:items-start gap-4 p-4 rounded-xl bg-zinc-900/60 border border-zinc-800/80">
            {/* Scannable High-Contrast QR Code */}
            <div className="relative group shrink-0 flex flex-col items-center">
              <div className="p-3 bg-white rounded-2xl shadow-xl shadow-cyan-950/20 border-2 border-cyan-500/40 transition-transform group-hover:scale-[1.02]">
                {qrDataUrl ? (
                  <img
                    src={qrDataUrl}
                    alt={`QR Code untuk alamat ${address}`}
                    className="w-36 h-36 sm:w-40 sm:h-40 object-contain rounded-lg"
                  />
                ) : (
                  <div className="w-36 h-36 sm:w-40 sm:h-40 flex items-center justify-center bg-zinc-100 rounded-lg">
                    <div className="h-6 w-6 animate-spin rounded-full border-2 border-cyan-600 border-t-transparent" />
                  </div>
                )}
              </div>

              {/* Enlarge zoom button */}
              <button
                type="button"
                onClick={() => setIsModalOpen(true)}
                className="mt-2 text-[11px] text-zinc-400 hover:text-cyan-400 inline-flex items-center gap-1 transition-colors cursor-pointer"
                title="Tampilkan QR code ukuran penuh"
              >
                <Maximize2 className="h-3 w-3" />
                <span>Perbesar Layar Penuh</span>
              </button>
            </div>

            {/* Address Details & Format Switcher */}
            <div className="flex-1 min-w-0 space-y-3 w-full text-center sm:text-left">
              {/* Format selection chips */}
              <div className="flex flex-wrap items-center justify-center sm:justify-start gap-1.5">
                <span className="text-[11px] text-zinc-400 font-medium mr-1">
                  Format QR:
                </span>
                <button
                  type="button"
                  onClick={() => setFormatType("address")}
                  className={`text-[11px] px-2.5 py-0.5 rounded-full font-mono transition-all cursor-pointer ${
                    formatType === "address"
                      ? "bg-cyan-500 text-zinc-950 font-bold shadow-sm"
                      : "bg-zinc-800 text-zinc-400 hover:text-zinc-200 border border-zinc-700"
                  }`}
                >
                  Raw Address
                </button>
                <button
                  type="button"
                  onClick={() => setFormatType("eip681")}
                  className={`text-[11px] px-2.5 py-0.5 rounded-full font-mono transition-all cursor-pointer ${
                    formatType === "eip681"
                      ? "bg-cyan-500 text-zinc-950 font-bold shadow-sm"
                      : "bg-zinc-800 text-zinc-400 hover:text-zinc-200 border border-zinc-700"
                  }`}
                  title="Format standar EIP-681 otomatis mengenali Chain ID 1001 di aplikasi Web3 wallet"
                >
                  EIP-681 Web3 URI
                </button>
              </div>

              {/* Full Address Display Box */}
              <div className="bg-zinc-950/90 rounded-xl p-3 border border-zinc-800/80 space-y-1">
                <div className="flex items-center justify-between text-[11px]">
                  <span className="text-zinc-400 flex items-center gap-1 font-medium">
                    <Wallet className="h-3 w-3 text-cyan-400" />
                    Alamat Wallet Anda:
                  </span>
                  <span className="text-[10px] font-mono text-emerald-400">
                    Siap Terima Dana
                  </span>
                </div>
                <div className="font-mono text-xs text-zinc-200 break-all select-all font-semibold leading-relaxed">
                  {address}
                </div>
              </div>

              {/* Action Buttons: Copy, Download, Share */}
              <div className="flex flex-wrap items-center gap-2 pt-1 justify-center sm:justify-start">
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  onClick={handleCopyAddress}
                  className="h-8 px-3 text-xs bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border-zinc-700 flex items-center gap-1.5"
                >
                  {copiedAddress ? (
                    <>
                      <Check className="h-3.5 w-3.5 text-emerald-400" />
                      <span className="text-emerald-400 font-semibold">Tersalin!</span>
                    </>
                  ) : (
                    <>
                      <Copy className="h-3.5 w-3.5 text-cyan-400" />
                      <span>Salin Alamat</span>
                    </>
                  )}
                </Button>

                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={handleDownloadQr}
                  className="h-8 px-3 text-xs border-zinc-700 text-zinc-300 hover:text-white hover:bg-zinc-800 flex items-center gap-1.5"
                  title="Simpan gambar QR Code ke galeri perangkat"
                >
                  <Download className="h-3.5 w-3.5 text-emerald-400" />
                  <span>Download QR (PNG)</span>
                </Button>

                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={handleShare}
                  className="h-8 px-3 text-xs border-zinc-700 text-zinc-300 hover:text-white hover:bg-zinc-800 flex items-center gap-1.5"
                >
                  <Share2 className="h-3.5 w-3.5 text-cyan-400" />
                  <span>Bagikan</span>
                </Button>
              </div>

              {/* Chain parameters notice */}
              <div className="flex items-center gap-2 text-[11px] text-zinc-400 pt-0.5">
                <span className="inline-block w-1.5 h-1.5 rounded-full bg-emerald-400" />
                <span>
                  Jaringan: <strong className="text-zinc-300">{networkName}</strong> (Chain ID: {chainId})
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Fullscreen QR Modal for Easy High-Resolution Scanning */}
      {isModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-xl animate-in fade-in duration-200">
          <div className="relative w-full max-w-sm rounded-3xl border border-zinc-800 bg-zinc-950 p-6 shadow-2xl shadow-cyan-950/30 text-center space-y-4">
            <button
              type="button"
              onClick={() => setIsModalOpen(false)}
              className="absolute top-4 right-4 p-2 rounded-xl text-zinc-400 hover:text-white hover:bg-zinc-800 transition-colors"
            >
              <X className="h-5 w-5" />
            </button>

            <div className="pt-2 flex flex-col items-center">
              <div className="p-2.5 rounded-2xl bg-cyan-950/80 border border-cyan-500/30 text-cyan-400 mb-2">
                <QrCode className="h-6 w-6" />
              </div>
              <h3 className="text-lg font-bold text-white">QR Code Wallet</h3>
              <p className="text-xs text-zinc-400">
                Arahkan kamera smartphone ke QR code di bawah
              </p>
            </div>

            {/* High-res White QR Box */}
            <div className="p-5 bg-white rounded-2xl shadow-xl border-4 border-cyan-500/30 inline-block mx-auto">
              {qrDataUrl && (
                <img
                  src={qrDataUrl}
                  alt={`QR Code untuk alamat ${address}`}
                  className="w-56 h-56 object-contain rounded-lg"
                />
              )}
            </div>

            {/* Address info */}
            <div className="font-mono text-xs text-zinc-300 bg-zinc-900 px-3 py-2 rounded-xl border border-zinc-800 break-all select-all font-semibold">
              {address}
            </div>

            {/* Actions */}
            <div className="flex items-center justify-center gap-2 pt-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={handleCopyAddress}
                className="text-xs h-9"
              >
                {copiedAddress ? (
                  <>
                    <Check className="h-4 w-4 text-emerald-400 mr-1.5" />
                    <span>Tersalin</span>
                  </>
                ) : (
                  <>
                    <Copy className="h-4 w-4 mr-1.5 text-cyan-400" />
                    <span>Copy Address</span>
                  </>
                )}
              </Button>
              <Button
                variant="subtle"
                size="sm"
                onClick={handleDownloadQr}
                className="text-xs h-9"
              >
                <Download className="h-4 w-4 mr-1.5" />
                <span>Simpan Gambar</span>
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
