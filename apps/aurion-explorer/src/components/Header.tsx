"use client";

import React from 'react';
import { ShieldCheck, RefreshCw, AlertTriangle, Droplet } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ThemeToggle } from '@/components/theme-toggle';

interface HeaderProps {
  isLive: boolean;
  onToggleLive: () => void;
  onManualRefresh: () => void;
  alertCount?: number;
  onOpenFaucet?: () => void;
  onSimulateQuorumDrop?: () => void;
  onSimulateRoundSkip?: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  isLive,
  onToggleLive,
  onManualRefresh,
  alertCount = 0,
  onOpenFaucet,
}) => {
  return (
    <header
      id="aurion-header"
      className="w-full flex flex-col md:flex-row items-start md:items-center justify-between py-3 px-4 bg-white dark:bg-[#0d1117] border-b border-[#d0d7de] dark:border-[#30363d] gap-3 transition-colors"
    >
      {/* Left: Brand Icon + Title & Subtitle */}
      <div className="flex items-center gap-3">
        {/* Aurion Geometric Logo */}
        <div className="relative flex items-center justify-center w-8 h-8 rounded bg-[#f6f8fa] dark:bg-[#161b22] border border-[#d0d7de] dark:border-[#30363d] shadow-xs overflow-hidden group">
          <svg
            viewBox="0 0 32 32"
            fill="none"
            className="w-6 h-6 transform transition-transform group-hover:scale-110"
          >
            <defs>
              <linearGradient id="aurionGrad" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stopColor="#58a6ff" />
                <stop offset="50%" stopColor="#388bfd" />
                <stop offset="100%" stopColor="#1f6feb" />
              </linearGradient>
              <linearGradient id="aurionNeon" x1="0%" y1="100%" x2="100%" y2="0%">
                <stop offset="0%" stopColor="#39c5cf" />
                <stop offset="100%" stopColor="#3fb950" />
              </linearGradient>
            </defs>
            <path
              d="M16 4 L28 26 L22 26 L16 14 L10 26 L4 26 Z"
              fill="url(#aurionGrad)"
            />
            <path
              d="M16 11 L21 21 L11 21 Z"
              className="fill-white dark:fill-[#0d1117]"
            />
            <path
              d="M13 18 L19 18 L16 12 Z"
              fill="url(#aurionNeon)"
            />
          </svg>
        </div>

        <div>
          <h1 className="text-sm md:text-base font-bold tracking-tight text-[#1f2328] dark:text-[#f0f6fc] font-mono flex items-center gap-2">
            AURION SOVEREIGN NETWORK — LIVE BEACON &amp; BLOCKCHAIN EXPLORER
          </h1>
          <p className="text-xs text-[#656d76] dark:text-[#8b949e] font-mono flex items-center gap-1.5 flex-wrap">
            <span>Live Decentralized Peer Mesh, BFT Consensus, and Block Ledger</span>
            <span className="hidden sm:inline text-[#d0d7de] dark:text-[#30363d]">•</span>
            <span className="hidden sm:inline-flex items-center gap-1 text-[11px] text-[#0969da] dark:text-[#39c5cf] bg-[#0969da]/10 dark:bg-[#39c5cf]/10 px-1.5 py-0.2 rounded border border-[#0969da]/30 dark:border-[#39c5cf]/30 font-medium">
              <ShieldCheck className="w-2.5 h-2.5" /> Byzantine Fault Tolerant
            </span>
          </p>
        </div>
      </div>

      {/* Right: Live Status Badges & Controls */}
      <div className="flex items-center gap-2 self-end md:self-auto flex-wrap">
        {/* Active BFT Alerts Badge */}
        {alertCount > 0 ? (
          <Badge
            variant="destructive"
            className="flex items-center gap-1.5 px-2 py-1 text-xs font-mono shadow-[0_0_10px_rgba(248,81,73,0.3)] animate-pulse"
          >
            <AlertTriangle className="w-3 h-3 text-[#cf222e] dark:text-[#f85149]" />
            <span className="font-bold">{alertCount} BFT Warning{alertCount > 1 ? 's' : ''}</span>
          </Badge>
        ) : (
          <Badge
            variant="secondary"
            className="hidden lg:flex items-center gap-1.5 px-2 py-1 text-[11px] font-mono border-[#d0d7de] dark:border-[#30363d] bg-[#f6f8fa] dark:bg-[#161b22] text-[#656d76] dark:text-[#8b949e]"
          >
            <span className="text-[#1a7f37] dark:text-[#3fb950] font-semibold">2/3+ Quorum</span>
            <span>Commit Rate 98.6%</span>
          </Badge>
        )}

        {/* Theme Switcher Toggle (Light & Dark Mode) */}
        <ThemeToggle />

        {onOpenFaucet && (
          <Button
            variant="outline"
            size="sm"
            onClick={onOpenFaucet}
            className="flex items-center gap-1.5 h-7 px-2.5 text-xs font-mono border-[#0969da]/30 dark:border-[#58a6ff]/30 text-[#0969da] dark:text-[#58a6ff] hover:bg-[#0969da]/10 dark:hover:bg-[#58a6ff]/10"
            title="Penyalur Koin Faucet Testnet"
          >
            <Droplet className="w-3 h-3 text-[#0969da] dark:text-[#58a6ff]" />
            <span>Faucet</span>
          </Button>
        )}

        <Button
          variant="outline"
          size="icon"
          onClick={onManualRefresh}
          title="Refresh Telemetry"
          className="h-7 w-7 border-[#d0d7de] dark:border-[#30363d] text-[#656d76] dark:text-[#8b949e] hover:text-[#1f2328] dark:hover:text-[#f0f6fc] hover:bg-[#f6f8fa] dark:hover:bg-[#161b22]"
        >
          <RefreshCw className="w-3 h-3" />
        </Button>

        <Button
          variant={isLive ? "live" : "paused"}
          size="sm"
          onClick={onToggleLive}
          title={isLive ? "Pause Live Stream" : "Resume Live Stream"}
          className={`cursor-pointer ${
            isLive
              ? "bg-[#dafbe1] dark:bg-[#238636]/20 text-[#1a7f37] dark:text-[#3fb950] border-[#4ac26b]/40 dark:border-[#238636]/60 shadow-xs"
              : "bg-[#eaeef2] dark:bg-[#21262d] text-[#656d76] dark:text-[#8b949e] border-[#d0d7de] dark:border-[#30363d]"
          }`}
        >
          <span className="relative flex h-2 w-2">
            {isLive && (
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#1a7f37] dark:bg-[#3fb950] opacity-75"></span>
            )}
            <span
              className={`relative inline-flex rounded-full h-2 w-2 ${
                isLive ? "bg-[#1a7f37] dark:bg-[#3fb950]" : "bg-[#656d76] dark:bg-[#8b949e]"
              }`}
            ></span>
          </span>
          <span className="font-semibold tracking-wide">
            {isLive ? "• LIVE" : "• PAUSED"}
          </span>
        </Button>

        <Badge
          variant="secondary"
          className="px-2.5 py-1 text-xs font-mono flex items-center gap-1.5 bg-[#dafbe1] dark:bg-[#161b22] text-[#1a7f37] dark:text-[#3fb950] border-[#4ac26b]/40 dark:border-[#30363d]"
        >
          <span className="inline-flex rounded-full h-2 w-2 bg-[#1a7f37] dark:bg-[#3fb950]"></span>
          <span className="font-semibold tracking-wide">• CONNECTED</span>
        </Badge>
      </div>
    </header>
  );
};
