export interface PaywallChallenge {
  channelId: bigint;
  amount: bigint;
}

export interface PaywallOptions {
  maxRetries?: number;
  onTicket?: (ticket: Uint8Array) => void;
}