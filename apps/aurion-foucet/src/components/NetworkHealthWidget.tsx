import React, { useEffect, useRef, useState, useCallback } from "react";
import {
  Activity,
  Clock,
  Zap,
  Wifi,
  CheckCircle2,
  RefreshCw,
  Server,
  Layers,
  ChevronDown,
} from "lucide-react";
import { NetworkHealthStats } from "../types";
import { Badge } from "./ui/badge";

interface NetworkHealthWidgetProps {
  onOpenExplorer?: () => void;
}

export const NetworkHealthWidget: React.FC<NetworkHealthWidgetProps> = ({
  onOpenExplorer,
}) => {
  const [stats, setStats] = useState<NetworkHealthStats>({
    blockTimeSec: 1.2,
    latencyMs: 24,
    blockHeight: 1483180,
    tps: 842,
    status: "optimal",
    lastUpdated: Date.now(),
  });

  const [isPolling, setIsPolling] = useState<boolean>(false);
  const [isDropdownOpen, setIsDropdownOpen] = useState<boolean>(false);
  const [pulseActive, setPulseActive] = useState<boolean>(false);
  const containerRef = useRef<HTMLDivElement | null>(null);

  // Mock polling function: simulates network telemetry ping to Aurion Testnet bootnodes
  const pollNetworkMetrics = useCallback(async () => {
    setIsPolling(true);
    setPulseActive(true);

    // Simulate network round-trip ping jitter (15ms - 42ms)
    const simulatedRoundTrip = Math.floor(Math.random() * 20) + 16;

    await new Promise((resolve) => setTimeout(resolve, simulatedRoundTrip));

    setStats((prev) => {
      // Calculate realistic micro-variances around 1.2s block time
      const variation = (Math.random() * 0.16 - 0.08); // -0.08 to +0.08s
      const newBlockTime = Math.max(1.05, Math.min(1.35, +(prev.blockTimeSec + variation).toFixed(2)));
      
      // Advance block height by 2 or 3 blocks every 3-second cycle
      const blockIncrement = Math.random() > 0.3 ? 3 : 2;
      const newBlockHeight = prev.blockHeight + blockIncrement;

      // Realistic TPS fluctuation around ~800-950
      const newTps = Math.floor(820 + Math.random() * 120);

      return {
        blockTimeSec: newBlockTime,
        latencyMs: simulatedRoundTrip,
        blockHeight: newBlockHeight,
        tps: newTps,
        status: simulatedRoundTrip < 70 ? "optimal" : "degraded",
        lastUpdated: Date.now(),
      };
    });

    setIsPolling(false);
    setTimeout(() => setPulseActive(false), 800);
  }, []);

  // Polling lifecycle: run every 3.5 seconds
  useEffect(() => {
    // Initial immediate poll
    pollNetworkMetrics();

    const intervalId = setInterval(() => {
      pollNetworkMetrics();
    }, 3500);

    return () => clearInterval(intervalId);
  }, [pollNetworkMetrics]);

  // Handle outside click to dismiss dropdown
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setIsDropdownOpen(false);
      }
    };

    if (isDropdownOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isDropdownOpen]);

  // Color coding helper for latency
  const getLatencyColor = (ms: number) => {
    if (ms < 50) return "text-emerald-400";
    if (ms < 100) return "text-amber-400";
    return "text-red-400";
  };

  const getLatencyBg = (ms: number) => {
    if (ms < 50) return "bg-emerald-500/15 border-emerald-500/30 text-emerald-300";
    if (ms < 100) return "bg-amber-500/15 border-amber-500/30 text-amber-300";
    return "bg-red-500/15 border-red-500/30 text-red-300";
  };

  return (
    <div ref={containerRef} className="relative inline-block text-left">
      {/* Interactive Trigger Button */}
      <button
        type="button"
        id="network-health-widget-btn"
        onClick={() => setIsDropdownOpen((prev) => !prev)}
        className={`group flex items-center gap-2 sm:gap-2.5 rounded-full border px-2.5 sm:px-3 py-1 text-xs font-mono transition-all cursor-pointer select-none ${
          isDropdownOpen
            ? "border-cyan-500/50 bg-cyan-950/40 shadow-sm shadow-cyan-500/20"
            : "border-zinc-800/90 bg-zinc-900/80 hover:bg-zinc-800/80 hover:border-zinc-700 text-zinc-300"
        }`}
        title="Aurion Testnet Network Health (Klik untuk detail metrik)"
      >
        {/* Pulse / Activity indicator */}
        <div className="flex items-center gap-1.5">
          <span className="relative flex h-2 w-2">
            <span
              className={`absolute inline-flex h-full w-full rounded-full bg-emerald-400 ${
                pulseActive ? "animate-ping opacity-90" : "opacity-40"
              }`}
            />
            <span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-500" />
          </span>

          <span className="font-sans font-semibold text-[11px] text-zinc-400 hidden lg:inline">
            Health:
          </span>
        </div>

        {/* Latency metric */}
        <div className="flex items-center gap-1">
          <Zap className="h-3 w-3 text-cyan-400" />
          <span className={`font-semibold ${getLatencyColor(stats.latencyMs)}`}>
            {stats.latencyMs}ms
          </span>
        </div>

        <span className="text-zinc-700 hidden xs:inline">•</span>

        {/* Block Time metric */}
        <div className="flex items-center gap-1">
          <Clock className="h-3 w-3 text-emerald-400" />
          <span className="text-zinc-200 font-semibold">
            {stats.blockTimeSec}s
          </span>
        </div>

        <ChevronDown
          className={`h-3 w-3 text-zinc-500 transition-transform duration-200 hidden sm:block ${
            isDropdownOpen ? "rotate-180 text-cyan-400" : "group-hover:text-zinc-300"
          }`}
        />
      </button>

      {/* Dropdown Popover with Detailed Network Health Telemetry */}
      {isDropdownOpen && (
        <div className="absolute right-0 mt-2 w-80 sm:w-88 rounded-2xl border border-zinc-800 bg-zinc-950/95 p-4 shadow-2xl shadow-cyan-950/40 backdrop-blur-xl z-50 animate-in fade-in-0 zoom-in-95 duration-150">
          {/* Header */}
          <div className="flex items-center justify-between pb-3 border-b border-zinc-800/80">
            <div className="flex items-center gap-2">
              <div className="p-1.5 rounded-lg bg-emerald-950/80 border border-emerald-500/30 text-emerald-400">
                <Activity className="h-4 w-4" />
              </div>
              <div>
                <h4 className="text-xs font-bold text-white tracking-wide uppercase font-mono">
                  Network Telemetry
                </h4>
                <p className="text-[10px] text-zinc-400">Aurion Testnet (Chain ID: 1001)</p>
              </div>
            </div>

            <button
              type="button"
              onClick={pollNetworkMetrics}
              disabled={isPolling}
              className="p-1.5 rounded-lg border border-zinc-800 bg-zinc-900/80 text-zinc-400 hover:text-cyan-400 hover:border-zinc-700 transition-all cursor-pointer disabled:opacity-50"
              title="Ping Ulang Jaringan Sekarang"
            >
              <RefreshCw
                className={`h-3.5 w-3.5 ${isPolling ? "animate-spin text-cyan-400" : ""}`}
              />
            </button>
          </div>

          {/* Grid of Key Metrics */}
          <div className="grid grid-cols-2 gap-2.5 my-3">
            {/* Block Time Card */}
            <div className="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800/80 space-y-1">
              <div className="flex items-center justify-between text-[11px] text-zinc-400">
                <span className="flex items-center gap-1 font-medium">
                  <Clock className="h-3 w-3 text-emerald-400" />
                  Block Time
                </span>
                <span className="text-[10px] text-emerald-400 font-mono">Fast</span>
              </div>
              <div className="flex items-baseline gap-1 font-mono">
                <span className="text-lg font-bold text-white tracking-tight">
                  {stats.blockTimeSec}
                </span>
                <span className="text-xs text-zinc-400">detik/blok</span>
              </div>
              <div className="text-[10px] text-zinc-500">Aurion PoS Instant Finality</div>
            </div>

            {/* Network Latency Card */}
            <div className="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800/80 space-y-1">
              <div className="flex items-center justify-between text-[11px] text-zinc-400">
                <span className="flex items-center gap-1 font-medium">
                  <Wifi className="h-3 w-3 text-cyan-400" />
                  Latency
                </span>
                <Badge
                  variant="outline"
                  className={`text-[9px] py-0 px-1 border ${getLatencyBg(stats.latencyMs)}`}
                >
                  {stats.latencyMs < 50 ? "Optimal" : "Normal"}
                </Badge>
              </div>
              <div className="flex items-baseline gap-1 font-mono">
                <span className={`text-lg font-bold ${getLatencyColor(stats.latencyMs)}`}>
                  {stats.latencyMs}
                </span>
                <span className="text-xs text-zinc-400">ms round-trip</span>
              </div>
              <div className="text-[10px] text-zinc-500">Global Bootnode Cluster</div>
            </div>

            {/* Current Block Height */}
            <div className="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800/80 space-y-1">
              <div className="flex items-center justify-between text-[11px] text-zinc-400">
                <span className="flex items-center gap-1 font-medium">
                  <Layers className="h-3 w-3 text-indigo-400" />
                  Block Height
                </span>
              </div>
              <div className="text-sm font-bold text-zinc-100 font-mono">
                #{stats.blockHeight.toLocaleString()}
              </div>
              <div className="text-[10px] text-zinc-500">Sinkronisasi Real-time</div>
            </div>

            {/* Network Throughput / TPS */}
            <div className="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800/80 space-y-1">
              <div className="flex items-center justify-between text-[11px] text-zinc-400">
                <span className="flex items-center gap-1 font-medium">
                  <Zap className="h-3 w-3 text-amber-400" />
                  Throughput
                </span>
              </div>
              <div className="flex items-baseline gap-1 font-mono">
                <span className="text-sm font-bold text-zinc-100">{stats.tps}</span>
                <span className="text-[10px] text-zinc-400">TPS</span>
              </div>
              <div className="text-[10px] text-zinc-500">Kapasitas Testnet</div>
            </div>
          </div>

          {/* Infrastructure status footer */}
          <div className="pt-2 border-t border-zinc-800/80 space-y-2">
            <div className="flex items-center justify-between text-[11px]">
              <span className="text-zinc-400 flex items-center gap-1.5">
                <Server className="h-3 w-3 text-emerald-400" />
                <span>Validator Nodes:</span>
              </span>
              <span className="font-mono text-emerald-300 font-medium">
                32/32 Active (100%)
              </span>
            </div>

            <div className="flex items-center justify-between text-[10px] text-zinc-500 pt-0.5">
              <span>Polling Otomatis (3.5s)</span>
              <span className="font-mono">
                {isPolling ? "Pinging node..." : "Live Active"}
              </span>
            </div>

            {onOpenExplorer && (
              <button
                type="button"
                onClick={() => {
                  setIsDropdownOpen(false);
                  onOpenExplorer();
                }}
                className="w-full mt-1 py-1.5 px-3 rounded-lg bg-zinc-900 hover:bg-zinc-800 border border-zinc-800 text-[11px] font-mono text-cyan-400 hover:text-cyan-300 transition-colors flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <span>Buka Aurion Block Explorer</span>
                <ChevronDown className="h-3 w-3 -rotate-90" />
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
};
