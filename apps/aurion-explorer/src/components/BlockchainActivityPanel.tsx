import React from 'react';
import Link from 'next/link';
import { Activity, ShieldCheck, ArrowRight, ExternalLink } from 'lucide-react';
import { BlockItem, TransactionItem } from '../types';
import { Card, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';

interface BlockchainActivityPanelProps {
  blocks: BlockItem[];
  transactions: TransactionItem[];
  onSelectBlock?: (block: BlockItem) => void;
  onSelectTransaction?: (tx: TransactionItem) => void;
}

export const BlockchainActivityPanel: React.FC<BlockchainActivityPanelProps> = ({
  blocks,
  transactions,
  onSelectBlock,
  onSelectTransaction,
}) => {
  return (
    <div
      id="panel-blockchain-activity"
      className="flex flex-col gap-2.5 h-full"
    >
      {/* 1. BLOK TERBARU (LATEST BLOCKS) - BFT Consensus & Proposer */}
      <Card
        id="section-latest-blocks"
        className="flex-1 overflow-hidden"
      >
        <CardHeader className="py-2 px-3">
          <CardTitle className="text-xs">
            <span>BLOK TERBARU</span>
            <span className="text-[#8b949e] font-normal">(LATEST BLOCKS)</span>
          </CardTitle>
          <Badge variant="success" className="gap-1 py-0.5 text-[10px]">
            <ShieldCheck className="w-3 h-3" />
            <span>BFT Quorum Finalized</span>
          </Badge>
        </CardHeader>

        <div className="flex-1 overflow-y-auto divide-y divide-[#30363d]/50 p-1">
          {blocks.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-6 text-center text-[#656d76] dark:text-[#8b949e] font-mono text-xs">
              <ShieldCheck className="w-5 h-5 mb-1.5 opacity-50 text-[#1a7f37] dark:text-[#3fb950]" />
              <span className="font-semibold text-[#1f2328] dark:text-[#f0f6fc]">Belum Ada Blok Baru</span>
              <span className="text-[11px] mt-0.5">Hanya Genesis Block #0 aktif di ledger lokal.</span>
            </div>
          ) : (
            blocks.slice(0, 4).map((block, idx) => {
              const proposerAddr = block.proposer || block.miner || '0x0000000000000000000000000000000000000000000000000000000000000000';
              const proposerLabel = block.proposerShort || block.minerShort || `${proposerAddr.slice(0, 8)}...`;
              const commitRate = block.quorumPercentage || '100%';

              return (
                <div
                  key={`${block.height}-${idx}`}
                  className="flex items-center justify-between px-2.5 py-2 text-xs font-mono hover:bg-[#21262d]/60 transition-colors rounded group"
                >
                  {/* Left: Block number & Proposer */}
                  <div className="flex items-center gap-2">
                    <Link
                      href={`/block/${block.height}`}
                      className="text-[#0969da] dark:text-[#58a6ff] font-semibold hover:underline flex items-center gap-1"
                      title={`Lihat detail blok #${block.height}`}
                    >
                      <span>Block {block.heightFormatted}</span>
                    </Link>
                    <span className="text-[#656d76] dark:text-[#8b949e] text-[11px] truncate max-w-[170px]">
                      oleh{' '}
                      <Link
                        href={`/account/${proposerAddr}`}
                        className="text-[#1f2328] dark:text-[#c9d1d9] hover:text-[#0969da] dark:hover:text-[#58a6ff] hover:underline"
                        title={`Lihat akun validator ${proposerAddr}`}
                      >
                        {proposerLabel}
                      </Link>
                    </span>
                  </div>

                  {/* Right: Specs (Txs, Commits, Time, Quick Modal) */}
                  <div className="flex items-center gap-2 sm:gap-3 text-[11px] text-[#656d76] dark:text-[#8b949e]">
                    <span>{block.txs} Txs</span>
                    <span className="hidden sm:inline text-[#1a7f37] dark:text-[#3fb950] bg-[#1a7f37]/10 dark:bg-[#3fb950]/10 px-1 rounded text-[10px]">
                      +2/3 ({commitRate})
                    </span>
                    <span className="text-[#656d76] dark:text-[#8b949e]/80 min-w-[50px] text-right">
                      {block.ageText}
                    </span>
                    <button
                      onClick={() => onSelectBlock?.(block)}
                      className="p-1 hover:text-[#1f2328] dark:hover:text-[#f0f6fc] text-[#656d76] dark:text-[#8b949e] rounded hover:bg-[#d0d7de]/40 dark:hover:bg-[#30363d]/50"
                      title="Modal Inspeksi Cepat"
                    >
                      <ExternalLink className="w-3 h-3" />
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </Card>

      {/* 2. TRANSAKSI TERBARU (LATEST TRANSACTIONS) */}
      <Card
        id="section-latest-transactions"
        className="flex-1 overflow-hidden"
      >
        <CardHeader className="py-2 px-3">
          <CardTitle className="text-xs">
            <span>TRANSAKSI TERBARU</span>
            <span className="text-[#656d76] dark:text-[#8b949e] font-normal">(LATEST TRANSACTIONS)</span>
          </CardTitle>
          <Activity className="w-3.5 h-3.5 text-[#0598ab] dark:text-[#39c5cf]" />
        </CardHeader>

        <div className="flex-1 overflow-y-auto divide-y divide-[#30363d]/50 p-1">
          {transactions.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-6 text-center text-[#656d76] dark:text-[#8b949e] font-mono text-xs">
              <Activity className="w-5 h-5 mb-1.5 opacity-50 text-[#0598ab] dark:text-[#39c5cf]" />
              <span className="font-semibold text-[#1f2328] dark:text-[#f0f6fc]">0 Transaksi</span>
              <span className="text-[11px] mt-0.5">Mempool bersih, belum ada transaksi baru yang dieksekusi.</span>
            </div>
          ) : (
            transactions.slice(0, 4).map((tx, idx) => {
              const barColors = [
                'bg-[#1a7f37] dark:bg-[#3fb950]',
                'bg-[#0598ab] dark:bg-[#39c5cf]',
                'bg-[#9a6700] dark:bg-[#d29922]',
                'bg-[#0969da] dark:bg-[#58a6ff]',
              ];
              const indicatorColor = barColors[idx % barColors.length];
              const senderAddr = tx.from;
              const receiverAddr = tx.to;
              const txHash = tx.id;

              return (
                <div
                  key={`${tx.id}-${idx}`}
                  className="flex items-center justify-between px-2.5 py-2 text-xs font-mono hover:bg-[#21262d]/60 transition-colors rounded group relative pl-3"
                >
                  {/* Colored Left Vertical Accent Bar */}
                  <div
                    className={`absolute left-1 top-2 bottom-2 w-[2px] rounded-full ${indicatorColor}`}
                  />

                  {/* Sender -> Receiver Hash with links */}
                  <div className="flex items-center gap-1.5 truncate max-w-[220px] md:max-w-[250px]">
                    <Link
                      href={`/tx/${txHash}`}
                      className="text-[#656d76] dark:text-[#8b949e] hover:text-[#0969da] dark:hover:text-[#58a6ff] hover:underline text-[11px] shrink-0"
                      title={`Detail transaksi 168-byte ${txHash}`}
                    >
                      Tx:
                    </Link>
                    <Link
                      href={`/account/${senderAddr}`}
                      className="text-[#0598ab] dark:text-[#39c5cf] text-[11px] font-medium truncate hover:underline"
                      title={`Akun Pengirim ${senderAddr}`}
                    >
                      {tx.displayFrom}
                    </Link>
                    <ArrowRight className="w-2.5 h-2.5 text-[#656d76] dark:text-[#8b949e] shrink-0" />
                    <Link
                      href={`/account/${receiverAddr}`}
                      className="text-[#1f2328] dark:text-[#f0f6fc] text-[11px] truncate hover:underline"
                      title={`Akun Penerima ${receiverAddr}`}
                    >
                      {tx.displayTo}
                    </Link>
                  </div>

                  {/* Amount & Gas Fee & Modal Trigger */}
                  <div className="flex items-center gap-2 sm:gap-3 text-[11px] text-[#656d76] dark:text-[#8b949e] shrink-0">
                    <Link
                      href={`/tx/${txHash}`}
                      className="text-[#1f2328] dark:text-[#f0f6fc] font-medium hover:text-[#0969da] dark:hover:text-[#58a6ff] hover:underline"
                    >
                      {tx.amount} AUR
                    </Link>
                    <button
                      onClick={() => onSelectTransaction?.(tx)}
                      className="p-1 hover:text-[#1f2328] dark:hover:text-[#f0f6fc] text-[#656d76] dark:text-[#8b949e] rounded hover:bg-[#d0d7de]/40 dark:hover:bg-[#30363d]/50"
                      title="Modal Inspeksi Cepat"
                    >
                      <ExternalLink className="w-3 h-3" />
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </Card>
    </div>
  );
};
