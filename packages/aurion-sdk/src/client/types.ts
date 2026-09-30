import type { Quanta } from "../core/quanta";
import type { Keypair } from "../crypto/keypair";

export interface TransferParams {
  signer: Keypair;
  recipient: string;
  amount: Quanta;
  fee?: Quanta;
  nonce?: bigint;
}

export interface BalanceParams {
  accountId: string;
}

export interface NonceProvider {
  getNonce(accountId: string): Promise<bigint> | bigint;
}