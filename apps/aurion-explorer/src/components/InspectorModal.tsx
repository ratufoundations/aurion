"use client";

import React from 'react';
import Link from 'next/link';
import { Check, Copy, ShieldCheck, Box, Activity, Server, ExternalLink } from 'lucide-react';
import { BlockItem, TransactionItem, PeerNode } from '../types';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface InspectorModalProps {
  selectedBlock: BlockItem | null;
  selectedTx: TransactionItem | null;
  selectedPeer: PeerNode | null;
  onClose: () => void;
}

export const InspectorModal: React.FC<InspectorModalProps> = ({
  selectedBlock,
  selectedTx,
  selectedPeer,
  onClose,
}) => {
  const [copied, setCopied] = React.useState(false);
  const isOpen = Boolean(selectedBlock || selectedTx || selectedPeer);

  const copyText = (val: string) => {
    navigator.clipboard.writeText(val);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const txHash = selectedTx
    ? (selectedTx.id.startsWith('0x') ? selectedTx.id : `0x9c4a${selectedTx.id.replace(/[^a-zA-Z0-9]/g, '')}`)
    : '';

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-lg w-full bg-[#161b22] border-[#30363d] text-[#c9d1d9] font-mono shadow-2xl p-0 overflow-hidden">
        {/* Modal Header */}
        <DialogHeader className="px-4 py-3 border-b border-[#30363d] bg-[#0d1117] flex flex-row items-center justify-between">
          <DialogTitle className="flex items-center gap-2 text-xs font-bold text-[#f0f6fc] uppercase tracking-wide">
            {selectedBlock && <Box className="w-4 h-4 text-[#58a6ff]" />}
            {selectedTx && <Activity className="w-4 h-4 text-[#39c5cf]" />}
            {selectedPeer && <Server className="w-4 h-4 text-[#3fb950]" />}
            <span>
              {selectedBlock
                ? `BFT Block Details ${selectedBlock.heightFormatted}`
                : selectedTx
                ? `Transaction Details`
                : `Peer Node Telemetry (BFT)`}
            </span>
          </DialogTitle>
        </DialogHeader>

        {/* Modal Content */}
        <div className="p-4 space-y-3 max-h-[75vh] overflow-y-auto">
          {/* Block Inspection */}
          {selectedBlock && (
            <>
              <div className="space-y-1">
                <div className="text-[11px] text-[#8b949e]">BLOCK HASH</div>
                <div className="bg-[#0d1117] p-2 rounded border border-[#30363d] text-[11px] text-[#58a6ff] break-all flex items-center justify-between gap-2">
                  <Link
                    href={`/block/${selectedBlock.height}`}
                    onClick={onClose}
                    className="hover:underline flex-1 truncate"
                  >
                    {selectedBlock.hash}
                  </Link>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => copyText(selectedBlock.hash)}
                    className="shrink-0 h-6 w-6 text-[#8b949e] hover:text-[#f0f6fc]"
                  >
                    {copied ? <Check className="w-3.5 h-3.5 text-[#3fb950]" /> : <Copy className="w-3.5 h-3.5" />}
                  </Button>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">PROPOSER VALIDATOR</span>
                  <Link
                    href={`/account/${selectedBlock.proposer || selectedBlock.miner || '0x8b3a8b27v5285a50ef03d7890bfa4312e'}`}
                    onClick={onClose}
                    className="text-[#58a6ff] font-semibold hover:underline block truncate"
                  >
                    {selectedBlock.proposerShort || selectedBlock.minerShort}
                  </Link>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">BFT CONSENSUS ROUND</span>
                  <span className="text-[#3fb950] font-semibold flex items-center gap-1">
                    <ShieldCheck className="w-3 h-3 text-[#3fb950]" />
                    Round {selectedBlock.bftRound ?? 0} (Instant Finality)
                  </span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">BFT QUORUM COMMITS</span>
                  <span className="text-[#39c5cf] font-semibold">
                    {selectedBlock.quorumSigs ?? 139}/142 (+2/3 Quorum: {selectedBlock.quorumPercentage ?? '97.9%'})
                  </span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">TRANSACTIONS</span>
                  <span className="text-[#f0f6fc] font-semibold">{selectedBlock.txs} Txs</span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">PAYLOAD SIZE</span>
                  <span className="text-[#f0f6fc] font-semibold">{selectedBlock.sizeKb} KB</span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">VALIDATOR REWARD</span>
                  <span className="text-[#d29922] font-semibold">{selectedBlock.reward} AUR</span>
                </div>
              </div>
            </>
          )}

          {/* Transaction Inspection */}
          {selectedTx && (
            <>
              <div className="space-y-1">
                <div className="text-[11px] text-[#8b949e]">TRANSACTION IDENTIFIER</div>
                <div className="bg-[#0d1117] p-2 rounded border border-[#30363d] text-[11px] text-[#39c5cf] break-all flex items-center justify-between gap-2">
                  <Link
                    href={`/tx/${txHash}`}
                    onClick={onClose}
                    className="hover:underline flex-1 truncate"
                  >
                    {txHash}
                  </Link>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => copyText(txHash)}
                    className="shrink-0 h-6 w-6 text-[#8b949e] hover:text-[#f0f6fc]"
                  >
                    {copied ? <Check className="w-3.5 h-3.5 text-[#3fb950]" /> : <Copy className="w-3.5 h-3.5" />}
                  </Button>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">AMOUNT TRANSFERRED</span>
                  <span className="text-[#d29922] font-semibold text-sm">{selectedTx.amount} koin AUR</span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">GAS ESTIMATE</span>
                  <span className="text-[#3fb950] font-semibold">{selectedTx.gasFee}</span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">SENDER ACCOUNT</span>
                  <Link
                    href={`/account/${selectedTx.from}`}
                    onClick={onClose}
                    className="text-[#39c5cf] hover:underline block truncate"
                  >
                    {selectedTx.displayFrom}
                  </Link>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">RECEIVER ACCOUNT</span>
                  <Link
                    href={`/account/${selectedTx.to || selectedTx.displayTo}`}
                    onClick={onClose}
                    className="text-[#f0f6fc] hover:underline block truncate"
                  >
                    {selectedTx.displayTo}
                  </Link>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d] col-span-2">
                  <span className="text-[#8b949e] block text-[10px]">BFT STATUS</span>
                  <span className="text-[#3fb950] font-semibold uppercase flex items-center gap-1">
                    <Check className="w-3 h-3" />
                    {selectedTx.status} (Finalized)
                  </span>
                </div>
              </div>
            </>
          )}

          {/* Peer Inspection */}
          {selectedPeer && (
            <>
              <div className="space-y-1">
                <div className="text-[11px] text-[#8b949e]">MULTIADDR LOCATOR</div>
                <div className="bg-[#0d1117] p-2 rounded border border-[#30363d] text-[11px] text-[#f0f6fc] break-all flex items-center justify-between gap-2">
                  <span>{selectedPeer.locator}</span>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => copyText(selectedPeer.locator)}
                    className="shrink-0 h-6 w-6 text-[#8b949e] hover:text-[#f0f6fc]"
                  >
                    {copied ? <Check className="w-3.5 h-3.5 text-[#3fb950]" /> : <Copy className="w-3.5 h-3.5" />}
                  </Button>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">CONSENSUS ROLE</span>
                  <Badge variant={selectedPeer.role === 'VALIDATOR' || (selectedPeer.role as string) === 'MINER' ? 'success' : 'warning'}>
                    {selectedPeer.role === 'VALIDATOR' || (selectedPeer.role as string) === 'MINER' ? 'VALIDATOR (BFT)' : selectedPeer.role}
                  </Badge>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">VOTING POWER</span>
                  <span className="text-[#39c5cf] font-bold">
                    {selectedPeer.votingPower ? `${selectedPeer.votingPower}% of Quorum` : '0% (Non-Voting Node)'}
                  </span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">ROUND-TRIP LATENCY</span>
                  <span className="text-[#58a6ff] font-bold">{selectedPeer.ping} ms</span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d]">
                  <span className="text-[#8b949e] block text-[10px]">GEO LOCATION</span>
                  <span className="text-[#f0f6fc]">{selectedPeer.city}, {selectedPeer.country}</span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d] col-span-2">
                  <span className="text-[#8b949e] block text-[10px]">CLIENT CONSENSUS ENGINE</span>
                  <span className="text-[#c9d1d9]">{selectedPeer.version} • Tendermint/Istanbul BFT Core</span>
                </div>
                <div className="bg-[#0d1117] p-2.5 rounded border border-[#30363d] col-span-2">
                  <span className="text-[#8b949e] block text-[10px]">NETWORK BANDWIDTH (IN / OUT)</span>
                  <span className="text-[#c9d1d9]">{selectedPeer.trafficString}</span>
                </div>
              </div>
            </>
          )}
        </div>

        {/* Footer with Direct Route Link & Close */}
        <DialogFooter className="px-4 py-2.5 bg-[#0d1117] border-t border-[#30363d] flex items-center justify-between sm:justify-between">
          <div>
            {selectedBlock && (
              <Link href={`/block/${selectedBlock.height}`} onClick={onClose}>
                <Button variant="outline" size="sm" className="h-7 text-xs gap-1.5 text-[#58a6ff] border-[#58a6ff]/40 hover:bg-[#58a6ff]/10">
                  <span>Lihat Rincian Blok Lengkap</span>
                  <ExternalLink className="w-3.5 h-3.5" />
                </Button>
              </Link>
            )}
            {selectedTx && (
              <Link href={`/tx/${txHash}`} onClick={onClose}>
                <Button variant="outline" size="sm" className="h-7 text-xs gap-1.5 text-[#39c5cf] border-[#39c5cf]/40 hover:bg-[#39c5cf]/10">
                  <span>Lihat Rincian Transaksi 168B</span>
                  <ExternalLink className="w-3.5 h-3.5" />
                </Button>
              </Link>
            )}
            {selectedPeer && (
              <Link href={`/account/${selectedPeer.locator}`} onClick={onClose}>
                <Button variant="outline" size="sm" className="h-7 text-xs gap-1.5 text-[#3fb950] border-[#3fb950]/40 hover:bg-[#3fb950]/10">
                  <span>Inspeksi Akun Node</span>
                  <ExternalLink className="w-3.5 h-3.5" />
                </Button>
              </Link>
            )}
          </div>
          <Button
            variant="secondary"
            size="sm"
            onClick={onClose}
            className="h-7 text-xs"
          >
            Tutup
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
