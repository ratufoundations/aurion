export interface TxReceipt {
  txHash: string;
  accepted: boolean;
  message: string;
}

export interface AccountResponse {
  address: string;
  balance: string;
  nonce: string;
}

export interface AurionTransport {
  readonly baseUrl: string;
  getBalance(chainId: string, accountId: string): Promise<string>;
  getAccount(chainId: string, accountId: string): Promise<AccountResponse>;
  submitEnvelope(chainId: string, envelope: Uint8Array): Promise<TxReceipt>;
}