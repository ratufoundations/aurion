import * as ed25519 from "@noble/ed25519";
import { Quanta } from "../core/quanta";
import { Keypair } from "../crypto/keypair";
import type { BalanceProof, ChannelId } from "./types";

export const TICKET_SIZE = 128;
export const PREIMAGE_SIZE = 64;

export function encodeBalanceProofPreimage(
  channelId: ChannelId,
  nonce: bigint,
  transferred: Quanta,
  senderPubkey: Uint8Array,
): Uint8Array {
  const buf = new Uint8Array(PREIMAGE_SIZE);
  const view = new DataView(buf.buffer);
  view.setBigUint64(0, channelId, true);
  view.setBigUint64(8, nonce, true);
  view.setBigUint64(16, transferred.toBigInt(), true);
  buf.set(senderPubkey, 32);
  return buf;
}

export function signBalanceProof(
  channelId: ChannelId,
  nonce: bigint,
  transferred: Quanta,
  keypair: Keypair,
): Uint8Array {
  const preimage = encodeBalanceProofPreimage(
    channelId,
    nonce,
    transferred,
    keypair.publicKey,
  );
  const signature = keypair.sign(preimage);
  const ticket = new Uint8Array(TICKET_SIZE);
  ticket.set(preimage, 0);
  ticket.set(signature, PREIMAGE_SIZE);
  return ticket;
}

export function decodeBalanceProof(buf: Uint8Array): BalanceProof {
  if (buf.length !== TICKET_SIZE) {
    throw new Error(
      `BalanceProof must be ${TICKET_SIZE} bytes, received ${buf.length}`,
    );
  }
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  const channelId = view.getBigUint64(0, true);
  const nonce = view.getBigUint64(8, true);
  const transferredAmount = view.getBigUint64(16, true);
  const senderPubkey = buf.slice(32, 64);
  const signature = buf.slice(64, 128);
  return {
    channelId,
    nonce,
    transferredAmount: Quanta.fromBigInt(transferredAmount),
    senderPubkey,
    signature,
  };
}

export function verifyBalanceProof(buf: Uint8Array): boolean {
  if (buf.length !== TICKET_SIZE) {
    return false;
  }
  const preimage = buf.slice(0, PREIMAGE_SIZE);
  const senderPubkey = buf.slice(32, 64);
  const signature = buf.slice(64, 128);
  return ed25519.verify(signature, preimage, senderPubkey);
}