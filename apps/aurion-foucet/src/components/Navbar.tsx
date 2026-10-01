import React from "react";
import { Sparkles, Globe, BookOpen, ExternalLink, ShieldCheck, Wallet } from "lucide-react";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { NetworkHealthWidget } from "./NetworkHealthWidget";

interface NavbarProps {
  onOpenExplorer: () => void;
  onOpenDocs: () => void;
  onConnectWallet?: () => void;
  connectedAddress?: string | null;
}

export const Navbar: React.FC<NavbarProps> = ({
  onOpenExplorer,
  onOpenDocs,
  onConnectWallet,
  connectedAddress,
}) => {
  return (
    <header className="sticky top-0 z-40 w-full border-b border-zinc-800/80 bg-zinc-950/80 backdrop-blur-xl">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-4 sm:px-6 lg:px-8">
        {/* Brand Logo & Name */}
        <div className="flex items-center gap-3">
          <div className="relative flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-cyan-500 via-emerald-500 to-indigo-600 p-[1.5px] shadow-lg shadow-cyan-500/20">
            <div className="flex h-full w-full items-center justify-center rounded-[10px] bg-zinc-950">
              <Sparkles className="h-5 w-5 text-cyan-400 animate-pulse" />
            </div>
          </div>
          <div className="flex flex-col">
            <div className="flex items-center gap-2">
              <span className="text-lg font-extrabold tracking-wider text-white font-mono">
                AURION
              </span>
              <Badge variant="cyan" className="px-2 py-0 text-[10px] uppercase font-bold tracking-wider">
                Testnet Faucet
              </Badge>
            </div>
            <span className="text-[11px] text-zinc-400 hidden sm:inline-block">
              Decentralized Developer Testnet Portal
            </span>
          </div>
        </div>

        {/* Center/Right Nav Actions */}
        <div className="flex items-center gap-2 sm:gap-3">
          {/* Real-time Network Health Widget */}
          <NetworkHealthWidget onOpenExplorer={onOpenExplorer} />

          {/* Chain ID Pill */}
          <div
            className="hidden sm:flex items-center gap-1.5 rounded-full border border-zinc-800 bg-zinc-900/60 px-2.5 py-1 text-xs font-mono text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer"
            title="Aurion Testnet Chain ID 1001"
            onClick={onOpenExplorer}
          >
            <span className="text-[10px] text-zinc-500 font-sans">Chain:</span>
            <span className="font-semibold text-cyan-300">1001</span>
          </div>

          {/* Quick Links */}
          <div className="hidden md:flex items-center gap-1.5">
            <Button
              variant="ghost"
              size="sm"
              onClick={onOpenExplorer}
              className="text-zinc-400 hover:text-white gap-1.5 text-xs h-8"
            >
              <Globe className="h-3.5 w-3.5 text-cyan-400" />
              Explorer
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={onOpenDocs}
              className="text-zinc-400 hover:text-white gap-1.5 text-xs h-8"
            >
              <BookOpen className="h-3.5 w-3.5 text-emerald-400" />
              Documentation
            </Button>
          </div>

          {/* Connect Wallet or Connected Indicator */}
          {onConnectWallet && (
            <Button
              variant="outline"
              size="sm"
              onClick={onConnectWallet}
              className="border-zinc-800 bg-zinc-900/80 hover:bg-zinc-800 text-xs font-mono gap-1.5 h-8 px-3 text-zinc-300"
            >
              <Wallet className="h-3.5 w-3.5 text-cyan-400" />
              {connectedAddress ? (
                <span>
                  {connectedAddress.slice(0, 6)}...{connectedAddress.slice(-4)}
                </span>
              ) : (
                <span className="hidden sm:inline">Connect Wallet</span>
              )}
              {connectedAddress && (
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
              )}
            </Button>
          )}
        </div>
      </div>
    </header>
  );
};
