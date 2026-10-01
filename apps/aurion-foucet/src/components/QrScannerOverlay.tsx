import React, { useEffect, useRef, useState } from "react";
import jsQR from "jsqr";
import {
  Camera,
  X,
  FlipHorizontal,
  Upload,
  AlertCircle,
  CheckCircle2,
  Sparkles,
  Zap,
  Image as ImageIcon,
  QrCode,
} from "lucide-react";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";

interface QrScannerOverlayProps {
  isOpen: boolean;
  onClose: () => void;
  onScanSuccess: (scannedAddress: string) => void;
}

// Clean and extract address from raw QR code content
export function extractAddressFromQr(raw: string): string | null {
  if (!raw) return null;
  const text = raw.trim();

  // Match 0x followed by 40 or 64 hexadecimal characters
  const match = text.match(/0x[a-fA-F0-9]{64}|0x[a-fA-F0-9]{40}/);
  if (match) {
    return match[0];
  }

  // If text itself is a 40 or 64 hex without 0x prefix
  const rawHexMatch = text.match(/\b[a-fA-F0-9]{40}\b|\b[a-fA-F0-9]{64}\b/);
  if (rawHexMatch) {
    return `0x${rawHexMatch[0]}`;
  }

  return null;
}

export const QrScannerOverlay: React.FC<QrScannerOverlayProps> = ({
  isOpen,
  onClose,
  onScanSuccess,
}) => {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const [stream, setStream] = useState<MediaStream | null>(null);
  const [facingMode, setFacingMode] = useState<"environment" | "user">("environment");
  const [hasMultipleCameras, setHasMultipleCameras] = useState<boolean>(false);
  const [cameraError, setCameraError] = useState<string | null>(null);
  const [isInitializing, setIsInitializing] = useState<boolean>(true);
  const [scannedResult, setScannedResult] = useState<string | null>(null);
  const animationFrameIdRef = useRef<number | null>(null);

  // Stop camera stream utility
  const stopCameraStream = () => {
    if (animationFrameIdRef.current) {
      cancelAnimationFrame(animationFrameIdRef.current);
      animationFrameIdRef.current = null;
    }
    if (stream) {
      stream.getTracks().forEach((track) => {
        try {
          track.stop();
        } catch (e) {}
      });
      setStream(null);
    }
    if (videoRef.current) {
      videoRef.current.srcObject = null;
    }
  };

  // Start video stream
  useEffect(() => {
    if (!isOpen) {
      stopCameraStream();
      setScannedResult(null);
      setCameraError(null);
      return;
    }

    let isMounted = true;
    setIsInitializing(true);
    setCameraError(null);
    setScannedResult(null);

    // Check available devices
    if (navigator.mediaDevices && navigator.mediaDevices.enumerateDevices) {
      navigator.mediaDevices
        .enumerateDevices()
        .then((devices) => {
          const videoDevices = devices.filter((d) => d.kind === "videoinput");
          if (isMounted) {
            setHasMultipleCameras(videoDevices.length > 1);
          }
        })
        .catch(() => {});
    }

    const startCamera = async () => {
      try {
        if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
          throw new Error("Perangkat kamera tidak didukung pada browser ini atau mode tidak aman (insecure context).");
        }

        const constraints: MediaStreamConstraints = {
          video: {
            facingMode: { ideal: facingMode },
            width: { ideal: 1280 },
            height: { ideal: 720 },
          },
          audio: false,
        };

        const mediaStream = await navigator.mediaDevices.getUserMedia(constraints);

        if (!isMounted) {
          mediaStream.getTracks().forEach((t) => t.stop());
          return;
        }

        setStream(mediaStream);
        if (videoRef.current) {
          videoRef.current.srcObject = mediaStream;
          videoRef.current.setAttribute("playsinline", "true"); // essential for iOS Safari
          await videoRef.current.play();
        }
        setIsInitializing(false);
      } catch (err: any) {
        if (!isMounted) return;
        setIsInitializing(false);
        console.warn("Camera init error:", err);
        if (err.name === "NotAllowedError" || err.name === "PermissionDeniedError") {
          setCameraError("Izin kamera ditolak. Berikan izin akses kamera di pengaturan browser Anda.");
        } else if (err.name === "NotFoundError" || err.name === "DevicesNotFoundError") {
          setCameraError("Kamera tidak ditemukan pada perangkat ini. Anda dapat mengunggah file foto QR code.");
        } else {
          setCameraError(err.message || "Gagal membuka kamera perangkat.");
        }
      }
    };

    startCamera();

    return () => {
      isMounted = false;
      stopCameraStream();
    };
  }, [isOpen, facingMode]);

  // Scan frame loop
  useEffect(() => {
    if (!isOpen || !stream || scannedResult) return;

    let isActive = true;

    const scanFrame = () => {
      if (!isActive) return;

      const video = videoRef.current;
      const canvas = canvasRef.current;

      if (video && canvas && video.readyState >= HTMLMediaElement.HAVE_ENOUGH_DATA) {
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (ctx) {
          canvas.width = video.videoWidth;
          canvas.height = video.videoHeight;
          ctx.drawImage(video, 0, 0, canvas.width, canvas.height);

          const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
          const code = jsQR(imageData.data, imageData.width, imageData.height, {
            inversionAttempts: "dontInvert",
          });

          if (code && code.data) {
            const extracted = extractAddressFromQr(code.data);
            if (extracted) {
              setScannedResult(extracted);
              // Trigger vibration haptic if supported
              if (navigator.vibrate) {
                navigator.vibrate(100);
              }
              setTimeout(() => {
                onScanSuccess(extracted);
                onClose();
              }, 600);
              return;
            }
          }
        }
      }

      animationFrameIdRef.current = requestAnimationFrame(scanFrame);
    };

    animationFrameIdRef.current = requestAnimationFrame(scanFrame);

    return () => {
      isActive = false;
      if (animationFrameIdRef.current) {
        cancelAnimationFrame(animationFrameIdRef.current);
      }
    };
  }, [isOpen, stream, scannedResult, onScanSuccess, onClose]);

  // Handle uploading QR Code from file/image
  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (event) => {
      const img = new Image();
      img.onload = () => {
        const canvas = document.createElement("canvas");
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (!ctx) return;
        canvas.width = img.width;
        canvas.height = img.height;
        ctx.drawImage(img, 0, 0, img.width, img.height);
        const imgData = ctx.getImageData(0, 0, canvas.width, canvas.height);
        const code = jsQR(imgData.data, imgData.width, imgData.height);
        if (code && code.data) {
          const extracted = extractAddressFromQr(code.data);
          if (extracted) {
            setScannedResult(extracted);
            setTimeout(() => {
              onScanSuccess(extracted);
              onClose();
            }, 600);
          } else {
            alert(`QR code terdeteksi (${code.data.slice(0, 30)}...) tetapi tidak mengandung format address Aurion / EVM yang valid.`);
          }
        } else {
          alert("Tidak dapat menemukan QR code pada gambar yang diunggah. Pastikan gambar jelas dan tidak blur.");
        }
      };
      img.src = event.target?.result as string;
    };
    reader.readAsDataURL(file);
  };

  // Toggle front/back camera
  const toggleCameraFacing = () => {
    setFacingMode((prev) => (prev === "environment" ? "user" : "environment"));
  };

  // Quick simulate sample QR scan for preview testing
  const handleSimulateScan = (addr: string) => {
    setScannedResult(addr);
    setTimeout(() => {
      onScanSuccess(addr);
      onClose();
    }, 400);
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-4 bg-black/85 backdrop-blur-xl animate-in fade-in duration-200">
      {/* Hidden file input */}
      <input
        type="file"
        ref={fileInputRef}
        accept="image/*"
        className="hidden"
        onChange={handleFileUpload}
      />

      {/* Hidden canvas for video analysis */}
      <canvas ref={canvasRef} className="hidden" />

      <div className="relative w-full max-w-md rounded-3xl border border-zinc-800 bg-zinc-950 p-5 sm:p-6 shadow-2xl shadow-cyan-950/20 text-zinc-100 overflow-hidden">
        {/* Ambient top glow */}
        <div className="absolute -top-24 left-1/2 -translate-x-1/2 w-60 h-40 bg-cyan-500/20 blur-3xl pointer-events-none rounded-full" />

        {/* Header */}
        <div className="flex items-center justify-between pb-4 border-b border-zinc-800/80">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-cyan-950/60 border border-cyan-500/30 text-cyan-400">
              <QrCode className="h-5 w-5" />
            </div>
            <div>
              <h3 className="text-base font-bold text-white tracking-tight">
                Pindai QR Code Wallet
              </h3>
              <p className="text-xs text-zinc-400">
                Arahkan kamera ke QR code wallet Aurion Anda
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-2 rounded-xl text-zinc-400 hover:text-white hover:bg-zinc-800 transition-colors"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Viewfinder Camera Stage */}
        <div className="relative my-4 aspect-square w-full rounded-2xl overflow-hidden bg-zinc-900/90 border border-zinc-800 flex items-center justify-center">
          {/* Live Video Feed */}
          <video
            ref={videoRef}
            className="h-full w-full object-cover"
            playsInline
            muted
          />

          {/* Viewfinder Target Mask */}
          <div className="absolute inset-0 pointer-events-none flex items-center justify-center">
            {/* Darkened overlay edges */}
            <div className="absolute inset-0 bg-black/30" />

            {/* Scanning square */}
            <div className="relative w-64 h-64 border-2 border-cyan-500/50 rounded-2xl shadow-[0_0_0_9999px_rgba(0,0,0,0.45)]">
              {/* Corner brackets */}
              <div className="absolute -top-1 -left-1 w-6 h-6 border-t-4 border-l-4 border-cyan-400 rounded-tl-lg" />
              <div className="absolute -top-1 -right-1 w-6 h-6 border-t-4 border-r-4 border-cyan-400 rounded-tr-lg" />
              <div className="absolute -bottom-1 -left-1 w-6 h-6 border-b-4 border-l-4 border-cyan-400 rounded-bl-lg" />
              <div className="absolute -bottom-1 -right-1 w-6 h-6 border-b-4 border-r-4 border-cyan-400 rounded-br-lg" />

              {/* Animated Laser Scanning Beam */}
              {!scannedResult && !cameraError && (
                <div className="absolute inset-x-0 h-0.5 bg-gradient-to-r from-transparent via-cyan-400 to-transparent shadow-[0_0_8px_#22d3ee] animate-pulse top-1/2 -translate-y-1/2 motion-safe:animate-bounce" />
              )}

              {/* Success highlight */}
              {scannedResult && (
                <div className="absolute inset-0 bg-emerald-500/20 backdrop-blur-[2px] rounded-2xl flex flex-col items-center justify-center gap-2 animate-in zoom-in-95">
                  <div className="h-12 w-12 rounded-full bg-emerald-500 flex items-center justify-center shadow-lg shadow-emerald-500/50">
                    <CheckCircle2 className="h-7 w-7 text-zinc-950" />
                  </div>
                  <Badge variant="emerald" className="font-mono text-xs">
                    Address Terdeteksi!
                  </Badge>
                </div>
              )}
            </div>
          </div>

          {/* Camera Loading State */}
          {isInitializing && !cameraError && (
            <div className="absolute inset-0 bg-zinc-950/80 backdrop-blur-sm flex flex-col items-center justify-center gap-3 p-4 text-center">
              <div className="h-8 w-8 animate-spin rounded-full border-2 border-cyan-500 border-t-transparent" />
              <p className="text-xs text-zinc-400 font-medium">
                Mengaktifkan sensor kamera perangkat...
              </p>
            </div>
          )}

          {/* Camera Error Fallback State */}
          {cameraError && (
            <div className="absolute inset-0 bg-zinc-950/95 p-6 flex flex-col items-center justify-center text-center gap-3">
              <div className="p-3 rounded-2xl bg-red-950/60 border border-red-500/30 text-red-400">
                <AlertCircle className="h-6 w-6" />
              </div>
              <div className="space-y-1 max-w-xs">
                <h4 className="text-sm font-bold text-white">Kamera Tidak Tersedia</h4>
                <p className="text-xs text-zinc-400 leading-relaxed">
                  {cameraError}
                </p>
              </div>

              <div className="pt-2 flex flex-col gap-2 w-full max-w-xs">
                <Button
                  variant="subtle"
                  size="sm"
                  onClick={() => fileInputRef.current?.click()}
                  className="h-10 text-xs font-semibold gap-2"
                >
                  <Upload className="h-4 w-4" />
                  <span>Pilih Foto QR dari Galeri</span>
                </Button>
              </div>
            </div>
          )}
        </div>

        {/* Camera Controls & Alternatives */}
        <div className="space-y-3">
          <div className="flex items-center justify-between gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => fileInputRef.current?.click()}
              className="flex-1 h-10 text-xs border-zinc-800 bg-zinc-900/80 hover:bg-zinc-800 text-zinc-300 gap-1.5"
            >
              <Upload className="h-3.5 w-3.5 text-cyan-400" />
              <span>Unggah Gambar QR</span>
            </Button>

            {hasMultipleCameras && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={toggleCameraFacing}
                className="h-10 px-3 text-xs border-zinc-800 bg-zinc-900/80 hover:bg-zinc-800 text-zinc-300 gap-1.5"
                title="Ganti Kamera Depan / Belakang"
              >
                <FlipHorizontal className="h-3.5 w-3.5" />
                <span className="hidden sm:inline">Ganti Kamera</span>
              </Button>
            )}
          </div>

          {/* Quick Mock Sample (helps when testing on desktop without webcam or physical QR) */}
          <div className="pt-2 border-t border-zinc-800/80 flex items-center justify-between text-xs">
            <span className="text-[11px] text-zinc-500">Uji Coba Cepat:</span>
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => handleSimulateScan("0x71C27aA5208b53C0D0f845A95B29B87F7D032849")}
                className="text-[11px] font-mono text-cyan-400 hover:underline cursor-pointer"
              >
                Simulasi Sample Address
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
