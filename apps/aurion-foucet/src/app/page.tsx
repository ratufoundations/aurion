"use client";

import React, { useState, useEffect, useCallback } from "react";
import confetti from "canvas-confetti";
import { toast, Toaster } from "sonner";
import {
  Sparkles,
  Terminal,
  Shield,
  Layers,
  ArrowRight,
  RefreshCw,
  Info,
} from "lucide-react";
import { ClaimResponse, ClaimSuccessResponse, RecentClaim } from "../types";
import { requestFaucetTokens } from "../lib/api";
import { Navbar } from "../components/Navbar";
import { FaucetCard } from "../components/FaucetCard";
import { SuccessCard } from "../components/SuccessCard";
import { ErrorAlert } from "../components/ErrorAlert";
import { InfoSection } from "../components/InfoSection";
import { RecentClaims } from "../components/RecentClaims";
import { ExplorerModal } from "../components/ExplorerModal";
import { DocsModal } from "../components/DocsModal";
import { Footer } from "../components/Footer";

// Initial mock recent claims
const INITIAL_RECENT_CLAIMS: RecentClaim[] = [
  {
    id: "claim-1",
    address: "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7",
    txHash: "0x4b7f92a1c0d5e834b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
    amount: 10,
    timestamp: Date.now() - 1000 * 60 * 4,
    status: "confirmed",
    gasFee: "0.00021 AUR",
  },
  {
    id: "claim-2",
    address: "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
    txHash: "0x9c3e21884f01cba63914a27e5849dfc821568294101e40a4daec91b2901a88b1",
    amount: 10,
    timestamp: Date.now() - 1000 * 60 * 18,
    status: "confirmed",
    gasFee: "0.00019 AUR",
  },
  {
    id: "claim-3",
    address: "0x90F79bf6EB2c4f870365E785982E1f101E93b906",
    txHash: "0x12a87c53d9e801b67284addd200126d90694b7f92a1c0d5e834b92dc18148a1d",
    amount: 10,
    timestamp: Date.now() - 1000 * 60 * 45,
    status: "confirmed",
    gasFee: "0.00023 AUR",
  },
];

export default function AurionFaucetPage() {
  const [address, setAddress] = useState<string>("");
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [successData, setSuccessData] = useState<ClaimSuccessResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | undefined>(undefined);
  const [retryAfterSeconds, setRetryAfterSeconds] = useState<number | undefined>(undefined);
  const [cooldownUntil, setCooldownUntil] = useState<number | null>(null);
  const [cooldownRemaining, setCooldownRemaining] = useState<number>(0);

  const [connectedAddress, setConnectedAddress] = useState<string | null>(null);
  const [recentClaims, setRecentClaims] = useState<RecentClaim[]>(INITIAL_RECENT_CLAIMS);

  // Modals state
  const [explorerOpen, setExplorerOpen] = useState<boolean>(false);
  const [selectedTxHash, setSelectedTxHash] = useState<string | null>(null);
  const [docsOpen, setDocsOpen] = useState<boolean>(false);

  // Load stored cooldown and recent claims from localStorage on mount
  useEffect(() => {
    try {
      const storedCooldown = localStorage.getItem("aurion_faucet_cooldown_until");
      if (storedCooldown) {
        const until = parseInt(storedCooldown, 10);
        if (until > Date.now()) {
          setCooldownUntil(until);
        } else {
          localStorage.removeItem("aurion_faucet_cooldown_until");
        }
      }

      const storedClaims = localStorage.getItem("aurion_recent_claims");
      if (storedClaims) {
        const parsed = JSON.parse(storedClaims);
        if (Array.isArray(parsed) && parsed.length > 0) {
          setRecentClaims(parsed);
        }
      }
    } catch (e) {
      // Ignore storage errors
    }
  }, []);

  // Cooldown countdown timer effect
  useEffect(() => {
    if (!cooldownUntil) {
      setCooldownRemaining(0);
      return;
    }

    const updateTimer = () => {
      const diff = Math.max(0, Math.ceil((cooldownUntil - Date.now()) / 1000));
      setCooldownRemaining(diff);
      if (diff <= 0) {
        setCooldownUntil(null);
        localStorage.removeItem("aurion_faucet_cooldown_until");
      }
    };

    updateTimer();
    const interval = setInterval(updateTimer, 1000);
    return () => clearInterval(interval);
  }, [cooldownUntil]);

  // Handle Token Claim Request
  const handleClaim = async () => {
    const trimmedAddress = address.trim();
    if (!trimmedAddress) {
      toast.error("Silakan masukkan alamat wallet Anda terlebih dahulu.");
      return;
    }

    // Reset previous feedback
    setError(null);
    setErrorCode(undefined);
    setSuccessData(null);
    setIsLoading(true);

    try {
      const response: ClaimResponse = await requestFaucetTokens(trimmedAddress);

      if (response.success) {
        // Success 200 OK
        setSuccessData(response);
        setIsLoading(false);

        // Confetti celebration
        try {
          confetti({
            particleCount: 80,
            spread: 70,
            origin: { y: 0.6 },
            colors: ["#06b6d4", "#10b981", "#6366f1", "#a855f7"],
          });
        } catch (e) {
          // Ignore confetti errors
        }

        // Sonner toast
        toast.success("10 AUR berhasil dikirim!", {
          description: `Tx: ${response.tx_hash.slice(0, 10)}...${response.tx_hash.slice(-8)} (Block #${response.block_height})`,
        });

        // Set local 24-hour cooldown
        const until = Date.now() + 24 * 60 * 60 * 1000;
        setCooldownUntil(until);
        try {
          localStorage.setItem("aurion_faucet_cooldown_until", until.toString());
        } catch (e) {}

        // Add to recent claims
        const newClaim: RecentClaim = {
          id: `claim-${Date.now()}`,
          address: trimmedAddress,
          txHash: response.tx_hash,
          amount: response.amount,
          timestamp: response.timestamp,
          status: "confirmed",
          gasFee: response.gas_fee || "0.00021 AUR",
        };

        setRecentClaims((prev) => {
          const updated = [newClaim, ...prev.slice(0, 9)];
          try {
            localStorage.setItem("aurion_recent_claims", JSON.stringify(updated));
          } catch (e) {}
          return updated;
        });
      } else {
        // Error handling (429, 500, 503, 400)
        setIsLoading(false);
        setError(response.error);
        setErrorCode(response.code);

        if (response.code === "COOLDOWN_ACTIVE" || response.retry_after_seconds) {
          const secs = response.retry_after_seconds || 86400;
          setRetryAfterSeconds(secs);
          const until = Date.now() + secs * 1000;
          setCooldownUntil(until);
          try {
            localStorage.setItem("aurion_faucet_cooldown_until", until.toString());
          } catch (e) {}

          toast.error("Cooldown Aktif", {
            description: response.error,
          });
        } else if (response.code === "NODE_UNAVAILABLE") {
          toast.error("Bootnode Error (503)", {
            description: response.error,
          });
        } else {
          toast.error("Permintaan Gagal", {
            description: response.error,
          });
        }
      }
    } catch (err: any) {
      setIsLoading(false);
      const msg = err?.message || "Terjadi kesalahan jaringan yang tidak terduga.";
      setError(msg);
      toast.error("Gangguan Jaringan", { description: msg });
    }
  };

  // Reset Cooldown for testing convenience
  const handleResetCooldown = async () => {
    setCooldownUntil(null);
    setCooldownRemaining(0);
    setError(null);
    setErrorCode(undefined);
    try {
      localStorage.removeItem("aurion_faucet_cooldown_until");
      await fetch("/api/v1/claim", { method: "DELETE" });
    } catch (e) {}
    toast.info("Cooldown telah di-reset (Mode Pengujian)");
  };

  // Open Explorer Modal
  const handleOpenExplorerWithHash = (hash: string) => {
    setSelectedTxHash(hash);
    setExplorerOpen(true);
  };

  // Connect Wallet Action
  const handleConnectWallet = async () => {
    if (typeof window !== "undefined" && (window as any).ethereum) {
      try {
        const accounts = await (window as any).ethereum.request({
          method: "eth_requestAccounts",
        });
        if (accounts && accounts.length > 0) {
          setConnectedAddress(accounts[0]);
          setAddress(accounts[0]);
          toast.success("Dompet terhubung!", {
            description: `${accounts[0].slice(0, 6)}...${accounts[0].slice(-4)}`,
          });
        }
      } catch (err: any) {
        toast.error("Koneksi dibatalkan", { description: err.message });
      }
    } else {
      // Mock fast connect for preview testing
      const sample = "0x71C27aA5208b53C0D0f845A95B29B87F7D032849";
      setConnectedAddress(sample);
      setAddress(sample);
      toast.info("Wallet Terhubung (Sample Testnet Account)", {
        description: `${sample.slice(0, 6)}...${sample.slice(-4)}`,
      });
    }
  };

  // Add Network to MetaMask / Web3 Wallet
  const handleAddNetworkToWallet = async () => {
    if (typeof window !== "undefined" && (window as any).ethereum) {
      try {
        await (window as any).ethereum.request({
          method: "wallet_addEthereumChain",
          params: [
            {
              chainId: "0x3E9", // 1001
              chainName: "Aurion Testnet",
              nativeCurrency: {
                name: "Aurion",
                symbol: "AUR",
                decimals: 18,
              },
              rpcUrls: ["https://rpc.testnet.aurion.network"],
              blockExplorerUrls: ["https://explorer.testnet.aurion.network"],
            },
          ],
        });
        toast.success("Jaringan Aurion Testnet berhasil ditambahkan ke dompet!");
      } catch (err: any) {
        toast.error("Gagal menambahkan jaringan", {
          description: err.message || "Permintaan ditolak.",
        });
      }
    } else {
      toast.info("Parameter Jaringan Aurion Testnet", {
        description: "Chain ID: 1001 • RPC: https://rpc.testnet.aurion.network • Simbol: AUR",
      });
    }
  };

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 flex flex-col justify-between selection:bg-cyan-500/20 selection:text-cyan-200">
      {/* Toast provider */}
      <Toaster
        theme="dark"
        position="top-right"
        toastOptions={{
          style: {
            background: "#09090b",
            border: "1px solid #27272a",
            color: "#f4f4f5",
          },
        }}
      />

      {/* Navigation Header */}
      <Navbar
        onOpenExplorer={() => handleOpenExplorerWithHash("0x4b7f92a1c0d5e834b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069")}
        onOpenDocs={() => setDocsOpen(true)}
        onConnectWallet={handleConnectWallet}
        connectedAddress={connectedAddress}
      />

      {/* Main Content Area */}
      <main className="flex-1 w-full max-w-6xl mx-auto px-4 sm:px-6 lg:px-8 py-8 sm:py-12 space-y-8">
        {/* Error / Cooldown Alert */}
        <ErrorAlert
          error={error}
          code={errorCode}
          retryAfterSeconds={cooldownRemaining}
          onClear={() => setError(null)}
          onResetCooldown={handleResetCooldown}
        />

        {/* Success Card Result */}
        {successData && (
          <SuccessCard
            data={successData}
            onDismiss={() => setSuccessData(null)}
            onOpenExplorer={handleOpenExplorerWithHash}
          />
        )}

        {/* Faucet Claim Card (Central Hero) */}
        <FaucetCard
          address={address}
          setAddress={setAddress}
          isLoading={isLoading}
          onClaim={handleClaim}
          isCooldownActive={cooldownRemaining > 0}
          cooldownRemainingSeconds={cooldownRemaining}
          reserveBalance={4850210}
        />

        {/* Quick Testing Simulation Bar */}
        <div className="w-full max-w-2xl mx-auto p-3 rounded-2xl bg-zinc-900/40 border border-zinc-800/80 flex flex-wrap items-center justify-between gap-3 text-xs">
          <div className="flex items-center gap-2 text-zinc-400">
            <Terminal className="h-4 w-4 text-cyan-400" />
            <span className="font-semibold text-zinc-300">Quick Testing Hub:</span>
          </div>

          <div className="flex items-center gap-2 flex-wrap">
            <button
              type="button"
              onClick={() => setAddress("0x71C27aA5208b53C0D0f845A95B29B87F7D032849")}
              className="px-2.5 py-1 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-300 transition-colors cursor-pointer"
              title="Isi dengan alamat valid untuk uji coba status 200 OK"
            >
              Address Valid (200)
            </button>
            <button
              type="button"
              onClick={() => setAddress("0x0000000000000000000000000000000000000503")}
              className="px-2.5 py-1 rounded-lg bg-red-950/40 border border-red-500/20 text-red-300 hover:bg-red-900/50 transition-colors cursor-pointer"
              title="Uji coba simulasi penolakan Bootnode 503"
            >
              Simulasi Node 503
            </button>
            {cooldownRemaining > 0 && (
              <button
                type="button"
                onClick={handleResetCooldown}
                className="px-2.5 py-1 rounded-lg bg-amber-950/40 border border-amber-500/20 text-amber-300 hover:bg-amber-900/50 transition-colors cursor-pointer flex items-center gap-1"
              >
                <RefreshCw className="h-3 w-3" />
                Reset Cooldown
              </button>
            )}
          </div>
        </div>

        {/* Recent Transactions Stream */}
        <RecentClaims
          claims={recentClaims}
          onOpenExplorer={handleOpenExplorerWithHash}
        />

        {/* 3 Information & FAQ Cards */}
        <InfoSection onAddNetworkToWallet={handleAddNetworkToWallet} />
      </main>

      {/* Modals */}
      <ExplorerModal
        open={explorerOpen}
        onOpenChange={setExplorerOpen}
        txHash={selectedTxHash}
      />
      <DocsModal
        open={docsOpen}
        onOpenChange={setDocsOpen}
      />

      {/* Footer */}
      <Footer />
    </div>
  );
}
