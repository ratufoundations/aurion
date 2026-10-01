import React from "react";
import { Activity, CheckCircle, ExternalLink, ArrowUpRight, Clock, Fuel } from "lucide-react";
import { RecentClaim } from "../types";
import { Card, CardHeader, CardTitle, CardContent } from "./ui/card";
import { Badge } from "./ui/badge";

interface RecentClaimsProps {
  claims: RecentClaim[];
  onOpenExplorer: (hash: string) => void;
}

export const RecentClaims: React.FC<RecentClaimsProps> = ({ claims, onOpenExplorer }) => {
  if (!claims || claims.length === 0) return null;

  const formatHashShort = (hash: string) => {
    return `${hash.slice(0, 8)}...${hash.slice(-6)}`;
  };

  const formatTimeAgo = (timestamp: number) => {
    const diffSec = Math.floor((Date.now() - timestamp) / 1000);
    if (diffSec < 60) return `${diffSec}d lalu`;
    const diffMin = Math.floor(diffSec / 60);
    if (diffMin < 60) return `${diffMin}m lalu`;
    const diffHours = Math.floor(diffMin / 60);
    return `${diffHours}j lalu`;
  };

  return (
    <div className="w-full max-w-4xl mx-auto mt-8">
      <Card className="border-zinc-800/80 bg-zinc-900/40">
        <CardHeader className="p-4 sm:p-5 pb-3 border-b border-zinc-800/60 flex flex-row items-center justify-between">
          <div className="flex items-center gap-2">
            <Activity className="h-4 w-4 text-cyan-400 animate-pulse" />
            <div>
              <CardTitle className="text-sm font-bold text-white">
                Aktivitas Transaksi Faucet Terbaru
              </CardTitle>
              <p className="text-[11px] text-zinc-400 mt-0.5">
                Transparansi on-chain real-time mencakup biaya gas (gas fee) dan hash blok
              </p>
            </div>
          </div>
          <Badge variant="cyan" className="font-mono text-[10px]">
            Live Stream
          </Badge>
        </CardHeader>
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead className="bg-zinc-950/60 text-zinc-400 font-semibold border-b border-zinc-800/60">
                <tr>
                  <th className="p-3 pl-4">Tx Hash</th>
                  <th className="p-3">Alamat Tujuan</th>
                  <th className="p-3">Jumlah</th>
                  <th className="p-3">Gas Fee</th>
                  <th className="p-3">Waktu</th>
                  <th className="p-3 pr-4 text-right">Status</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-zinc-800/60 font-mono text-[11px]">
                {claims.map((claim) => (
                  <tr
                    key={claim.id}
                    className="hover:bg-zinc-800/30 transition-colors group cursor-pointer"
                    onClick={() => onOpenExplorer(claim.txHash)}
                    title={`Klik untuk melihat detail transaksi ${claim.txHash} di Aurion Explorer`}
                  >
                    <td className="p-3 pl-4 text-cyan-400 group-hover:underline flex items-center gap-1">
                      <span>{formatHashShort(claim.txHash)}</span>
                      <ArrowUpRight className="h-3 w-3 opacity-0 group-hover:opacity-100 transition-opacity" />
                    </td>
                    <td className="p-3 text-zinc-300">
                      {formatHashShort(claim.address)}
                    </td>
                    <td className="p-3 text-emerald-400 font-bold">
                      +{claim.amount} AUR
                    </td>
                    <td className="p-3">
                      <span className="inline-flex items-center gap-1 rounded-md bg-zinc-800/70 px-2 py-0.5 border border-zinc-700/50 text-[10.5px] text-zinc-300 font-medium group-hover:border-cyan-500/30 transition-colors">
                        <Fuel className="h-2.5 w-2.5 text-cyan-400/90" />
                        <span>{claim.gasFee || "0.00021 AUR"}</span>
                      </span>
                    </td>
                    <td className="p-3 text-zinc-500 font-sans">
                      {formatTimeAgo(claim.timestamp)}
                    </td>
                    <td className="p-3 pr-4 text-right font-sans">
                      <span className="inline-flex items-center gap-1 text-[11px] text-emerald-400 bg-emerald-950/40 px-2 py-0.5 rounded-md border border-emerald-500/20">
                        <CheckCircle className="h-2.5 w-2.5" />
                        Sukses
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};
