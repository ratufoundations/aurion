import { Quanta, DEFAULT_CHAIN_ID } from "@aurion/sdk";

export interface NodeStatus {
  chain_id: number;
  block_height: number;
  validator_count: number;
  required_quorum: number;
  latest_state_root: string;
  epoch_index?: number;
  is_synced?: boolean;
}

export interface BlockHeader {
  height: number;
  hash: string;
  prev_hash: string;
  state_root: string;
  tx_root?: string;
  proposer: string;
  timestamp: number;
  qc_signers?: string[];
  tx_count?: number;
  size_bytes?: number;
}

export interface TransactionDetail {
  hash: string;
  sender: string;
  receiver: string;
  amount_quanta: bigint;
  amount_aur: string;
  network_fee_quanta: bigint;
  network_fee_aur: string;
  nonce: number;
  block_height: number;
  status: "confirmed" | "pending";
  timestamp: number;
  ed25519_signature: string; // 64-byte hex (128 hex chars)
}

export interface BlockDetail extends BlockHeader {
  round: number;
  transactions: TransactionDetail[];
  quorum_sigs: number;
  total_validators: number;
  reward_aur: string;
}

export interface AccountInfo {
  address: string;
  balanceQuanta: bigint;
  balanceFormatted: string;
  rawQuantaString: string;
  nonce: number;
}

export class AurionRpcError extends Error {
  constructor(message: string, public code?: number, public details?: unknown) {
    super(message);
    this.name = "AurionRpcError";
  }
}

export class AurionRpcClient {
  readonly rpcUrl: string;
  readonly chainId: number;
  private requestId = 1;

  constructor(
    rpcUrl = process.env.NEXT_PUBLIC_AURION_RPC_URL || "http://127.0.0.1:8545",
    chainId = Number(process.env.NEXT_PUBLIC_AURION_CHAIN_ID || DEFAULT_CHAIN_ID || 1001)
  ) {
    this.rpcUrl = rpcUrl.replace(/\/+$/, "");
    this.chainId = chainId;
  }

  /**
   * Eksekusi panggilan JSON-RPC 2.0 atau fallback HTTP REST
   */
  private async call<T>(method: string, params: unknown[] = []): Promise<T> {
    const payload = {
      jsonrpc: "2.0",
      id: this.requestId++,
      method,
      params,
    };

    try {
      const response = await fetch(this.rpcUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal: AbortSignal.timeout(4000),
      });

      if (!response.ok) {
        throw new AurionRpcError(
          `RPC HTTP ${response.status}: ${response.statusText}`,
          response.status
        );
      }

      const json = await response.json();
      if (json.error) {
        throw new AurionRpcError(
          json.error.message || "Unknown JSON-RPC error",
          json.error.code,
          json.error.data
        );
      }

      return json.result as T;
    } catch (err: unknown) {
      if (err instanceof AurionRpcError) throw err;
      throw new AurionRpcError(
        `Gagal terhubung ke simpul RPC Aurion (${this.rpcUrl}): ${err instanceof Error ? err.message : String(err)}`
      );
    }
  }

  /**
   * Pengecekan koneksi cepat (ping/status)
   */
  async checkConnection(): Promise<boolean> {
    try {
      await this.getStatus();
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Mengambil status simpul: chain_id, block_height, validator_count, required_quorum, state_root
   */
  async getStatus(): Promise<NodeStatus> {
    try {
      return await this.call<NodeStatus>("aurion_status", []);
    } catch (rpcErr) {
      try {
        const restUrl = `${this.rpcUrl}/aurion/${this.chainId}/status`;
        const res = await fetch(restUrl, {
          method: "GET",
          headers: { Accept: "application/json" },
          signal: AbortSignal.timeout(3000),
        });
        if (res.ok) {
          const data = await res.json();
          return {
            chain_id: data.chain_id ?? this.chainId,
            block_height: data.block_height ?? data.height ?? 0,
            validator_count: data.validator_count ?? 142,
            required_quorum: data.required_quorum ?? 95,
            latest_state_root: data.latest_state_root ?? data.state_root ?? "0x0000000000000000000000000000000000000000000000000000000000000000",
            epoch_index: data.epoch_index ?? 0,
            is_synced: data.is_synced ?? true,
          };
        }
      } catch {
        // Fallback error
      }
      throw rpcErr;
    }
  }

  /**
   * Mengambil daftar blok terbaru (116B header + QC signers)
   */
  async getLatestBlocks(limit = 10): Promise<BlockHeader[]> {
    try {
      return await this.call<BlockHeader[]>("aurion_getLatestBlocks", [limit]);
    } catch (rpcErr) {
      try {
        const restUrl = `${this.rpcUrl}/aurion/${this.chainId}/blocks?limit=${limit}`;
        const res = await fetch(restUrl, {
          method: "GET",
          headers: { Accept: "application/json" },
          signal: AbortSignal.timeout(3000),
        });
        if (res.ok) {
          return await res.json();
        }
      } catch {
        // Fallback error
      }
      throw rpcErr;
    }
  }

  /**
   * Mengambil detail blok spesifik berdasarkan tinggi atau hash
   */
  async getBlock(heightOrHash: string | number): Promise<BlockDetail> {
    try {
      const isHeight = typeof heightOrHash === "number" || /^\d+$/.test(String(heightOrHash));
      const method = isHeight ? "aurion_getBlockByHeight" : "aurion_getBlockByHash";
      const param = isHeight ? Number(heightOrHash) : String(heightOrHash);
      return await this.call<BlockDetail>(method, [param]);
    } catch (rpcErr) {
      try {
        const restUrl = `${this.rpcUrl}/aurion/${this.chainId}/block/${heightOrHash}`;
        const res = await fetch(restUrl, {
          method: "GET",
          headers: { Accept: "application/json" },
          signal: AbortSignal.timeout(3000),
        });
        if (res.ok) {
          return await res.json();
        }
      } catch {
        // Fallback error
      }
      throw rpcErr;
    }
  }

  /**
   * Mengambil detail transaksi 168-byte berdasarkan hash
   */
  async getTransaction(hash: string): Promise<TransactionDetail> {
    try {
      return await this.call<TransactionDetail>("aurion_getTransactionByHash", [hash]);
    } catch (rpcErr) {
      try {
        const restUrl = `${this.rpcUrl}/aurion/${this.chainId}/tx/${hash}`;
        const res = await fetch(restUrl, {
          method: "GET",
          headers: { Accept: "application/json" },
          signal: AbortSignal.timeout(3000),
        });
        if (res.ok) {
          return await res.json();
        }
      } catch {
        // Fallback error
      }
      throw rpcErr;
    }
  }

  /**
   * Mengambil informasi akun, saldo Quanta (bigint) & nonce
   */
  async getAccount(address: string): Promise<AccountInfo> {
    try {
      const result = await this.call<{
        address: string;
        balance: string | number | bigint;
        nonce: number;
      }>("aurion_getAccount", [address]);

      const quanta = typeof result.balance === "bigint"
        ? Quanta.fromBigInt(result.balance)
        : Quanta.fromQuantaString(String(result.balance || "0"));

      return {
        address: result.address || address,
        balanceQuanta: quanta.toBigInt(),
        balanceFormatted: `${quanta.toAur()} AUR`,
        rawQuantaString: `${quanta.toString()} Quanta`,
        nonce: result.nonce || 0,
      };
    } catch (rpcErr) {
      try {
        const restUrl = `${this.rpcUrl}/aurion/${this.chainId}/account/${address}`;
        const res = await fetch(restUrl, {
          method: "GET",
          headers: { Accept: "application/json" },
          signal: AbortSignal.timeout(3000),
        });
        if (res.ok) {
          const data = await res.json();
          const quanta = Quanta.fromQuantaString(String(data.balance || "0"));
          return {
            address,
            balanceQuanta: quanta.toBigInt(),
            balanceFormatted: `${quanta.toAur()} AUR`,
            rawQuantaString: `${quanta.toString()} Quanta`,
            nonce: data.nonce ?? 0,
          };
        }
      } catch {
        // Fallback error
      }
      throw rpcErr;
    }
  }

  /**
   * Permintaan dana Quanta uji coba dari genesis treasury faucet simpul lokal
   */
  async requestFaucet(address: string): Promise<{
    success: boolean;
    recipient: string;
    amount: number;
    amount_aur: string;
    amount_quanta: string;
    symbol: string;
    tx_hash: string;
    block_height: number;
    timestamp: number;
    message: string;
  }> {
    return await this.call("aur_requestFaucet", [address]);
  }
}

export const rpcClient = new AurionRpcClient();
