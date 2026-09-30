import { Quanta } from "../core/quanta";

export type ChannelId = bigint;

export type ChannelStatus = "open" | "challenging" | "settled";

export interface ChannelConfig {
  channelId: ChannelId;
  deposit: Quanta;
  senderPubkey: Uint8Array;
  receiverPubkey: Uint8Array;
}

export interface TicketState {
  channelId: ChannelId;
  nonce: bigint;
  transferredAmount: Quanta;
  senderPubkey: Uint8Array;
}

export interface BalanceProof {
  channelId: ChannelId;
  nonce: bigint;
  transferredAmount: Quanta;
  senderPubkey: Uint8Array;
  signature: Uint8Array;
}