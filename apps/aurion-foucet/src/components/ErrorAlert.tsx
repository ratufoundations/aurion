import React from "react";
import { AlertCircle, Clock, ServerCrash, RefreshCw, XCircle } from "lucide-react";
import { Alert, AlertTitle, AlertDescription } from "./ui/alert";
import { Button } from "./ui/button";

interface ErrorAlertProps {
  error: string | null;
  code?: string;
  retryAfterSeconds?: number;
  onClear: () => void;
  onResetCooldown?: () => void;
}

export const ErrorAlert: React.FC<ErrorAlertProps> = ({
  error,
  code,
  retryAfterSeconds,
  onClear,
  onResetCooldown,
}) => {
  if (!error) return null;

  const isCooldown = code === "COOLDOWN_ACTIVE" || error.toLowerCase().includes("cooldown") || error.includes("429");
  const isNodeError = code === "NODE_UNAVAILABLE" || error.toLowerCase().includes("bootnode") || error.includes("503");

  const formatCountdown = (secs?: number) => {
    if (!secs || secs <= 0) return null;
    const hours = Math.floor(secs / 3600);
    const mins = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    return `${hours.toString().padStart(2, "0")}:${mins
      .toString()
      .padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  };

  return (
    <div className="w-full max-w-2xl mx-auto animate-in fade-in slide-in-from-top-2 duration-300">
      <Alert
        variant={isCooldown ? "warning" : "destructive"}
        className="rounded-2xl border bg-zinc-950/90 backdrop-blur-xl shadow-xl"
      >
        {isCooldown ? (
          <Clock className="h-5 w-5 text-amber-400" />
        ) : isNodeError ? (
          <ServerCrash className="h-5 w-5 text-red-400" />
        ) : (
          <XCircle className="h-5 w-5 text-red-400" />
        )}

        <div className="flex flex-col sm:flex-row sm:items-start justify-between gap-3">
          <div className="space-y-1 pr-2">
            <AlertTitle className="text-sm font-bold tracking-tight">
              {isCooldown
                ? "Batas Permintaan Tercapai (Cooldown 24 Jam)"
                : isNodeError
                ? "Gangguan Koneksi Bootnode Cluster (HTTP 503)"
                : "Permintaan Faucet Ditolak"}
            </AlertTitle>
            <AlertDescription className="text-xs leading-relaxed text-zinc-300">
              {error}
            </AlertDescription>

            {isCooldown && retryAfterSeconds && retryAfterSeconds > 0 && (
              <div className="pt-2 flex items-center gap-2">
                <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-amber-500/10 border border-amber-500/30 text-amber-300 font-mono text-xs font-semibold">
                  <Clock className="h-3 w-3 animate-spin" />
                  Sisa Cooldown: {formatCountdown(retryAfterSeconds)}
                </span>
              </div>
            )}
          </div>

          <div className="flex items-center gap-2 shrink-0 self-end sm:self-start">
            {isCooldown && onResetCooldown && (
              <Button
                variant="outline"
                size="sm"
                onClick={onResetCooldown}
                className="h-8 px-2.5 text-[11px] border-amber-500/30 text-amber-300 hover:bg-amber-950/40 hover:text-amber-200"
                title="Reset cooldown lokal untuk kebutuhan pengujian UI"
              >
                <RefreshCw className="h-3 w-3 mr-1" />
                Reset (Test Mode)
              </Button>
            )}

            <Button
              variant="ghost"
              size="sm"
              onClick={onClear}
              className="h-8 px-2 text-xs text-zinc-400 hover:text-white"
            >
              Tutup
            </Button>
          </div>
        </div>
      </Alert>
    </div>
  );
};
