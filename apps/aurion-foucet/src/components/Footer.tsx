import React from "react";
import { Sparkles, Github, Globe, FileText, ShieldAlert, Heart } from "lucide-react";

export const Footer: React.FC = () => {
  return (
    <footer className="w-full border-t border-zinc-800/80 bg-zinc-950/60 mt-16 py-10">
      <div className="mx-auto max-w-6xl px-4 sm:px-6 lg:px-8 space-y-6">
        <div className="flex flex-col sm:flex-row items-center justify-between gap-4">
          <div className="flex items-center gap-2.5">
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-zinc-900 border border-zinc-800 text-cyan-400">
              <Sparkles className="h-4 w-4" />
            </div>
            <span className="font-mono font-bold text-white tracking-wider text-sm">
              AURION NETWORK
            </span>
            <span className="text-zinc-600">•</span>
            <span className="text-xs text-zinc-400">
              Testnet Infrastructure Portal
            </span>
          </div>

          <div className="flex items-center gap-4 text-xs text-zinc-400">
            <a
              href="https://github.com/aurion-network"
              target="_blank"
              rel="noreferrer"
              className="flex items-center gap-1.5 hover:text-white transition-colors"
            >
              <Github className="h-4 w-4" />
              <span>GitHub</span>
            </a>
            <a
              href="https://docs.testnet.aurion.network"
              target="_blank"
              rel="noreferrer"
              className="flex items-center gap-1.5 hover:text-white transition-colors"
            >
              <FileText className="h-4 w-4" />
              <span>Docs</span>
            </a>
            <a
              href="https://explorer.testnet.aurion.network"
              target="_blank"
              rel="noreferrer"
              className="flex items-center gap-1.5 hover:text-white transition-colors"
            >
              <Globe className="h-4 w-4" />
              <span>Explorer</span>
            </a>
          </div>
        </div>

        {/* Disclaimer Warning Box */}
        <div className="rounded-xl border border-zinc-800/80 bg-zinc-900/30 p-3.5 flex items-start gap-2.5 text-xs text-zinc-400 leading-relaxed">
          <ShieldAlert className="h-4 w-4 text-amber-400 shrink-0 mt-0.5" />
          <p>
            <strong>Pemberitahuan Resmi:</strong> Token AUR yang didistribusikan melalui portal faucet ini adalah token testnet murni untuk tujuan pengujian teknis, simulasi transaksi, dan deployment smart contract di lingkungan Aurion Testnet (Chain ID 1001). Token ini <strong>tidak memiliki nilai moneter nyata</strong>, tidak dapat diperjualbelikan, dan tidak dapat ditukarkan dengan mata uang fiat atau aset kripto riil lainnya.
          </p>
        </div>

        <div className="flex flex-col sm:flex-row items-center justify-between gap-2 text-[11px] text-zinc-500 pt-2">
          <span>
            © {new Date().getFullYear()} Aurion Network Foundation. All rights reserved.
          </span>
          <span className="font-mono">
            Genesis Pool Version 1.4.2 • PoS Testnet v0.9.8
          </span>
        </div>
      </div>
    </footer>
  );
};
