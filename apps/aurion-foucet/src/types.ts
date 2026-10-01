export interface ClaimRequest {
  target_address: string;
}

export interface ClaimSuccessResponse {
  success: true;
  message: string;
  tx_hash: string;
  amount: number;
  symbol: string;
  block_height: number;
  timestamp: number;
  recipient: string;
  gas_fee: string;
}

export interface ClaimErrorResponse {
  success: false;
  error: string;
  code?: 'COOLDOWN_ACTIVE' | 'INVALID_ADDRESS' | 'NODE_UNAVAILABLE' | 'RATE_LIMITED' | 'RESERVE_DEPLETED';
  retry_after_seconds?: number;
  cooldown_until?: number;
}

export type ClaimResponse = ClaimSuccessResponse | ClaimErrorResponse;

export interface RecentClaim {
  id: string;
  address: string;
  txHash: string;
  amount: number;
  timestamp: number;
  status: "confirmed" | "pending" | "failed";
  gasFee?: string;
}

export interface NetworkConfig {
  networkName: string;
  chainId: number;
  chainIdHex: string;
  rpcUrl: string;
  symbol: string;
  decimals: number;
  explorerUrl: string;
}

export interface NetworkHealthStats {
  blockTimeSec: number;
  latencyMs: number;
  blockHeight: number;
  tps: number;
  status: "optimal" | "degraded" | "syncing";
  lastUpdated: number;
}
