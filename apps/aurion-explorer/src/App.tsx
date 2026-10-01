"use client";

import React, { useState, useEffect, useCallback } from 'react';
import { Header } from './components/Header';
import { MetricsGrid } from './components/MetricsGrid';
import { PeerMapPanel } from './components/PeerMapPanel';
import { BlockchainActivityPanel } from './components/BlockchainActivityPanel';
import { PeerTelemetryTable } from './components/PeerTelemetryTable';
import { InspectorModal } from './components/InspectorModal';
import { ToastNotificationSystem } from './components/ToastNotificationSystem';
import { FaucetModal } from './components/FaucetModal';
import {
  PeerNode,
  BlockItem,
  TransactionItem,
  MetricCardData,
  BFTAlert,
} from './types';
import { rpcClient } from '@/lib/rpc';

export default function App() {
  const [isLive, setIsLive] = useState(true);
  const [isRpcConnected, setIsRpcConnected] = useState(false);
  const [isFaucetOpen, setIsFaucetOpen] = useState(false);
  const [chainId, setChainId] = useState<number>(1001);
  const [peers, setPeers] = useState<PeerNode[]>([]);
  const [blocks, setBlocks] = useState<BlockItem[]>([]);
  const [transactions, setTransactions] = useState<TransactionItem[]>([]);

  const [metrics, setMetrics] = useState<MetricCardData>({
    totalPeers: 1,
    activeValidators: 1,
    activeMiners: 1,
    fullNodes: 0,
    blockHeight: 0,
    bootnodeUptime: '0m',
    performanceRate: 100.0,
    tps: 0.0,
    mempool: 0,
    bftQuorum: '100%',
    bftRound: 0,
    epochIndex: 0,
    stateRoot: '0x0000000000000000000000000000000000000000000000000000000000000000',
  });

  // Modal inspection states
  const [selectedBlock, setSelectedBlock] = useState<BlockItem | null>(null);
  const [selectedTx, setSelectedTx] = useState<TransactionItem | null>(null);
  const [selectedPeer, setSelectedPeer] = useState<PeerNode | null>(null);

  // Toast notification alerts state (deterministic initial state for SSR)
  const [alerts, setAlerts] = useState<BFTAlert[]>([
    {
      id: 'initial-alert-beacon',
      type: 'BFT_WARNING',
      title: 'BFT WATCHDOG: ACTIVE',
      message: 'Byzantine Fault Tolerance watchdog aktif. Memantau konsensus BFT & kuorum 2/3+ validator.',
      severity: 'info',
      timestamp: '00:00:00',
      timeMs: 1727727000000,
      autoCloseMs: 8000,
    },
  ]);

  // Handler to dismiss individual alert
  const handleDismissAlert = useCallback((id: string) => {
    setAlerts((prev) => prev.filter((a) => a.id !== id));
  }, []);

  // Handler to clear all alerts
  const handleClearAllAlerts = useCallback(() => {
    setAlerts([]);
  }, []);

  // Trigger Quorum Drop Warning (<67%)
  const triggerQuorumDropWarning = useCallback((customQuorum?: number) => {
    const val = customQuorum ?? Number((58.4 + Math.random() * 5).toFixed(1));
    const activeVals = 1;
    const newAlert: BFTAlert = {
      id: `alert-quorum-${Date.now()}`,
      type: 'QUORUM_DROP',
      title: 'BFT WARNING: QUORUM DEFICIT (<67%)',
      message: `Critical Byzantine risk: Active quorum dropped to ${val}% (${activeVals} validator). Supermajority (+2/3) breached, consensus at risk of halting!`,
      severity: 'critical',
      timestamp: new Date().toLocaleTimeString(),
      timeMs: Date.now(),
      quorumValue: val,
      autoCloseMs: 12000,
    };

    setAlerts((prev) => [newAlert, ...prev.slice(0, 4)]);
  }, []);

  // Trigger Round Skip Warning
  const triggerRoundSkipWarning = useCallback((customRound?: number) => {
    const nextRound = customRound ?? ((metrics.bftRound || 0) + 1);
    const newAlert: BFTAlert = {
      id: `alert-skip-${Date.now()}`,
      type: 'ROUND_SKIP',
      title: 'BFT WARNING: ROUND-SKIPPING DETECTED',
      message: `Proposer timeout on Round #${nextRound - 1}. Quorum failed to collect 2/3+ precommits within timeout. Skipped to Round #${nextRound}!`,
      severity: 'warning',
      timestamp: new Date().toLocaleTimeString(),
      timeMs: Date.now(),
      roundNumber: nextRound,
      autoCloseMs: 10000,
    };

    setAlerts((prev) => [newAlert, ...prev.slice(0, 4)]);
  }, [metrics.bftRound]);

  // Real RPC Polling function
  const checkRpc = useCallback(async () => {
    try {
      const status = await rpcClient.getStatus();
      setIsRpcConnected(true);
      if (status.chain_id) {
        setChainId(status.chain_id);
      }
      const currentHeight = status.block_height ?? 0;
      setMetrics((prev) => ({
        ...prev,
        blockHeight: currentHeight,
        activeValidators: status.validator_count || 1,
        activeMiners: status.validator_count || 1,
        totalPeers: status.validator_count || 1,
        fullNodes: 0,
        bftQuorum: status.validator_count
          ? `${((status.required_quorum / status.validator_count) * 100).toFixed(1)}%`
          : '100%',
        epochIndex: status.epoch_index ?? Math.floor(currentHeight / 100),
        stateRoot: status.latest_state_root ?? prev.stateRoot,
      }));

      // Ambil blok riil dari RPC
      try {
        const rpcBlocks = await rpcClient.getLatestBlocks(10);
        if (rpcBlocks && rpcBlocks.length > 0) {
          const mappedBlocks: BlockItem[] = rpcBlocks.map((b) => {
            const ageSec = Math.max(0, Math.floor((Date.now() - b.timestamp) / 1000));
            return {
              height: b.height,
              heightFormatted: `#${b.height.toLocaleString()}`,
              proposer: b.proposer,
              proposerShort: b.proposer.length > 10 ? `${b.proposer.slice(0, 8)}...` : b.proposer,
              miner: b.proposer,
              minerShort: b.proposer.length > 10 ? `${b.proposer.slice(0, 8)}...` : b.proposer,
              txs: b.tx_count ?? 0,
              sizeKb: b.size_bytes ? Number((b.size_bytes / 1024).toFixed(1)) : 0.1,
              ageSeconds: ageSec,
              ageText: ageSec === 0 ? 'Baru saja' : `${ageSec}d lalu`,
              hash: b.hash,
              reward: 0,
              bftRound: 0,
              quorumSigs: b.qc_signers?.length ?? 1,
              quorumPercentage: '100%',
            };
          });
          setBlocks(mappedBlocks);

          // Ambil transaksi riil dari blok yang memiliki transaksi
          const collectedTxs: TransactionItem[] = [];
          for (const b of rpcBlocks) {
            if (b.tx_count && b.tx_count > 0) {
              try {
                const detail = await rpcClient.getBlock(b.height);
                if (detail && detail.transactions) {
                  for (const t of detail.transactions) {
                    collectedTxs.push({
                      id: t.hash,
                      from: t.sender,
                      to: t.receiver,
                      displayFrom: `${t.sender.slice(0, 8)}...`,
                      displayTo: `${t.receiver.slice(0, 8)}...`,
                      amount: Number(t.amount_aur.replace(/[^0-9.]/g, '')),
                      gasFee: t.network_fee_aur,
                      timeAgo: 'Confirmed',
                      type: 'transfer',
                      status: 'confirmed',
                    });
                  }
                }
              } catch {
                // Abaikan kesalahan detail blok
              }
            }
          }
          setTransactions(collectedTxs);
        } else {
          setBlocks([]);
          setTransactions([]);
        }
      } catch {
        // Biarkan state bersih
      }
    } catch {
      setIsRpcConnected(false);
    }
  }, []);

  // Poll RPC simpul lokal setiap 3 detik
  useEffect(() => {
    checkRpc();
    const interval = setInterval(checkRpc, 3000);
    return () => clearInterval(interval);
  }, [checkRpc]);

  const handleManualRefresh = () => {
    checkRpc();
  };

  return (
    <div className="min-h-screen bg-[#f6f8fa] dark:bg-[#0d1117] text-[#24292f] dark:text-[#c9d1d9] flex flex-col font-mono selection:bg-[#0969da]/20 dark:selection:bg-[#388bfd]/30 relative transition-colors">
      {/* 1. HEADER with Alert Badge, Theme Switcher & Faucet Button */}
      <Header
        isLive={isLive}
        onToggleLive={() => setIsLive(!isLive)}
        onManualRefresh={handleManualRefresh}
        alertCount={alerts.length}
        onOpenFaucet={() => setIsFaucetOpen(true)}
        onSimulateQuorumDrop={() => triggerQuorumDropWarning()}
        onSimulateRoundSkip={() => triggerRoundSkipWarning()}
      />

      {/* 2. Banner Status Simpul BFT (RPC 127.0.0.1:8545) */}
      <div
        className={`px-4 py-1.5 flex items-center justify-between text-xs font-mono border-b transition-colors ${
          isRpcConnected
            ? "bg-[#dafbe1] dark:bg-[#238636]/15 text-[#1a7f37] dark:text-[#3fb950] border-[#4ac26b]/30"
            : "bg-[#fff8c5] dark:bg-[#d29922]/15 text-[#9a6700] dark:text-[#d29922] border-[#d4a72c]/30"
        }`}
      >
        <div className="flex items-center gap-2">
          <span className="relative flex h-2 w-2">
            <span
              className={`relative inline-flex rounded-full h-2 w-2 ${
                isRpcConnected ? "bg-[#1a7f37] dark:bg-[#3fb950]" : "bg-[#9a6700] dark:bg-[#d29922]"
              }`}
            />
          </span>
          <span className="font-semibold tracking-wide">
            {isRpcConnected
              ? `SIMPUL BFT TERHUBUNG (127.0.0.1:8545) — SINKRONISASI DATA RIIL AKTIF • CHAIN ID ${chainId}`
              : "MENUNGGU KONEKSI KE SIMPUL BFT (127.0.0.1:8545)..."}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[11px] opacity-80 hidden md:inline">
            {isRpcConnected ? "RPC Mode: JSON-RPC 2.0" : "Status: Menghubungkan..."}
          </span>
          <button
            onClick={checkRpc}
            className="text-[10px] px-2 py-0.5 rounded border border-current hover:opacity-80 transition-opacity cursor-pointer font-bold"
          >
            CEK RPC
          </button>
        </div>
      </div>

      {/* 3. Real-Time Toast Notification System */}
      <ToastNotificationSystem
        alerts={alerts}
        onDismiss={handleDismissAlert}
        onClearAll={handleClearAllAlerts}
        onSimulateQuorumDrop={() => triggerQuorumDropWarning()}
        onSimulateRoundSkip={() => triggerRoundSkipWarning()}
      />

      {/* 4. GRID METRIK (Baris Atas - BFT Network Stats & Recharts 10m Health Monitor) */}
      <MetricsGrid
        metrics={metrics}
        onQuorumDrop={(val) => triggerQuorumDropWarning(val)}
        onRoundSkip={(round) => triggerRoundSkipWarning(round)}
      />

      {/* 5. MIDDLE DUAL PANELS (Left: 3D Mesh Topology / Peer Map, Right: Blockchain Activity) */}
      <main className="flex-1 p-3 pt-0 grid grid-cols-1 lg:grid-cols-12 gap-2.5">
        {/* PANEL KIRI: 3D MESH TOPOLOGY / PEER MAP (7 cols on lg) */}
        <div className="lg:col-span-7 xl:col-span-7 h-full min-h-[360px]">
          <PeerMapPanel
            avgLatency={0}
            validatorCount={metrics.activeValidators}
            minerCount={metrics.activeValidators}
            fullNodeCount={metrics.fullNodes}
          />
        </div>

        {/* PANEL KANAN: BLOCKCHAIN ACTIVITY (5 cols on lg) */}
        <div className="lg:col-span-5 xl:col-span-5 h-full">
          <BlockchainActivityPanel
            blocks={blocks}
            transactions={transactions}
            onSelectBlock={(block) => setSelectedBlock(block)}
            onSelectTransaction={(tx) => setSelectedTx(tx)}
          />
        </div>

        {/* 6. PANEL BAWAH: PEER TELEMETRY TABLE (12 cols) */}
        <div className="col-span-12">
          <PeerTelemetryTable
            peers={peers}
            onSelectPeer={(peer) => setSelectedPeer(peer)}
          />
        </div>
      </main>

      {/* Modal Inspector for inspecting BFT blocks, transactions, or peer nodes */}
      <InspectorModal
        selectedBlock={selectedBlock}
        selectedTx={selectedTx}
        selectedPeer={selectedPeer}
        onClose={() => {
          setSelectedBlock(null);
          setSelectedTx(null);
          setSelectedPeer(null);
        }}
      />

      {/* Modal Faucet Aurion Testnet */}
      <FaucetModal
        isOpen={isFaucetOpen}
        onClose={() => setIsFaucetOpen(false)}
        onSuccess={checkRpc}
      />
    </div>
  );
}
