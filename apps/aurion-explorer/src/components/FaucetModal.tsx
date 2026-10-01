"use client";

import React, { useState } from 'react';
import Link from 'next/link';
import { X, Droplet, ArrowRight, CheckCircle2, AlertCircle, Copy, Check, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { rpcClient } from '@/lib/rpc';

interface FaucetModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess?: () => void;
}

export const FaucetModal: React.FC<FaucetModalProps> = ({
  isOpen,
  onClose,
  onSuccess,
}) => {
  const [address, setAddress] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    tx_hash: string;
    block_height: number;
    amount_aur: string;
    recipient: string;
  } | null>(null);
  const [copied, setCopied] = useState(false);

  if (!isOpen) return null;

  const handleRandomAddress = () => {
    // Generate valid 32-byte hex address for testing
    const hex = Array.from({ length: 32 }, () =>
      Math.floor(Math.random() * 256).toString(16).padStart(2, '0')
    ).join('');
    setAddress(`0x${hex}`);
    setError(null);
  };

  const handleClaim = async (e: React.FormEvent) => {
    e.preventDefault();
    const cleanAddr = address.trim();
    if (!cleanAddr) {
      setError('Masukkan alamat publik penerima terlebih dahulu.');
      return;
    }

    setLoading(true);
    setError(null);
    setResult(null);

    try {
      const res = await rpcClient.requestFaucet(cleanAddr);
      if (res.success) {
        setResult({
          tx_hash: res.tx_hash,
          block_height: res.block_height,
          amount_aur: res.amount_aur || '10.0 AUR',
          recipient: res.recipient || cleanAddr,
        });
        if (onSuccess) {
          onSuccess();
        }
      } else {
        setError(res.message || 'Gagal mengeksekusi permintaan faucet.');
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Gagal mengirim permintaan faucet: ${msg}`);
    } finally {
      setLoading(false);
    }
  };

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-xs font-mono">
      <div className="relative w-full max-w-lg bg-white dark:bg-[#161b22] border border-[#d0d7de] dark:border-[#30363d] rounded-lg shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[#d0d7de] dark:border-[#30363d] bg-[#f6f8fa] dark:bg-[#0d1117]">
          <div className="flex items-center gap-2">
            <div className="p-1.5 rounded-full bg-[#0969da]/10 dark:bg-[#58a6ff]/10 text-[#0969da] dark:text-[#58a6ff]">
              <Droplet className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-[#1f2328] dark:text-[#f0f6fc]">
                AURION TESTNET FAUCET
              </h2>
              <p className="text-[11px] text-[#656d76] dark:text-[#8b949e]">
                Penyalur Quanta Genesis Otentik (10 AUR)
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded text-[#656d76] dark:text-[#8b949e] hover:text-[#1f2328] dark:hover:text-[#f0f6fc] hover:bg-[#d0d7de]/40 dark:hover:bg-[#30363d]/50"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-5 flex flex-col gap-4">
          <p className="text-xs text-[#656d76] dark:text-[#8b949e] leading-relaxed">
            Permintaan faucet akan ditandatangani langsung oleh otoritas Treasury Genesis dan dikirimkan ke mesin konsensus lokal untuk dimasukkan ke blok berikutnya secara permanen.
          </p>

          <form onSubmit={handleClaim} className="flex flex-col gap-3">
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-xs">
                <label className="font-semibold text-[#1f2328] dark:text-[#f0f6fc]">
                  Alamat Penerima (Recipient Address)
                </label>
                <button
                  type="button"
                  onClick={handleRandomAddress}
                  className="text-[11px] text-[#0969da] dark:text-[#58a6ff] hover:underline"
                >
                  Buat Alamat Acak
                </button>
              </div>
              <Input
                placeholder="0x... (32-byte public key hex)"
                value={address}
                onChange={(e) => setAddress(e.target.value)}
                className="font-mono text-xs bg-white dark:bg-[#0d1117] border-[#d0d7de] dark:border-[#30363d]"
                disabled={loading}
              />
            </div>

            {error && (
              <div className="flex items-start gap-2 p-3 text-xs rounded bg-[#ffebe9] dark:bg-[#490202]/30 border border-[#ff8182]/40 text-[#cf222e] dark:text-[#ff7b72]">
                <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
                <span>{error}</span>
              </div>
            )}

            {result && (
              <div className="flex flex-col gap-2 p-3 text-xs rounded bg-[#dafbe1] dark:bg-[#04260f]/40 border border-[#4ac26b]/40 text-[#1a7f37] dark:text-[#3fb950]">
                <div className="flex items-center gap-1.5 font-bold">
                  <CheckCircle2 className="w-4 h-4 shrink-0" />
                  <span>Transaksi Faucet Berhasil Dikomit!</span>
                </div>
                <div className="flex flex-col gap-1 text-[11px] text-[#1f2328] dark:text-[#c9d1d9] mt-1">
                  <div className="flex justify-between">
                    <span className="text-[#656d76] dark:text-[#8b949e]">Jumlah:</span>
                    <span className="font-bold text-[#1a7f37] dark:text-[#3fb950]">{result.amount_aur}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-[#656d76] dark:text-[#8b949e]">Blok:</span>
                    <Link
                      href={`/block/${result.block_height}`}
                      className="font-bold text-[#0969da] dark:text-[#58a6ff] hover:underline"
                    >
                      #{result.block_height}
                    </Link>
                  </div>
                  <div className="flex flex-col gap-0.5 mt-1">
                    <span className="text-[#656d76] dark:text-[#8b949e]">Tx Hash:</span>
                    <div className="flex items-center justify-between gap-1 p-1 rounded bg-black/5 dark:bg-black/30 border border-black/10 dark:border-white/10 font-mono text-[10px]">
                      <Link
                        href={`/tx/${result.tx_hash}`}
                        className="truncate text-[#0969da] dark:text-[#58a6ff] hover:underline"
                      >
                        {result.tx_hash}
                      </Link>
                      <button
                        type="button"
                        onClick={() => handleCopy(result.tx_hash)}
                        className="p-1 hover:text-[#1f2328] dark:hover:text-[#f0f6fc]"
                        title="Copy Tx Hash"
                      >
                        {copied ? <Check className="w-3 h-3 text-[#3fb950]" /> : <Copy className="w-3 h-3" />}
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            )}

            <div className="flex items-center justify-end gap-2 mt-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={onClose}
                disabled={loading}
              >
                Tutup
              </Button>
              <Button
                type="submit"
                size="sm"
                disabled={loading}
                className="bg-[#0969da] hover:bg-[#0969da]/90 text-white font-mono text-xs gap-1.5"
              >
                {loading ? (
                  <>
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    <span>Mengeksekusi...</span>
                  </>
                ) : (
                  <>
                    <span>Kirim 10 AUR</span>
                    <ArrowRight className="w-3.5 h-3.5" />
                  </>
                )}
              </Button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
};
