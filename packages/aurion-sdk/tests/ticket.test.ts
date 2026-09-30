import { describe, expect, it } from "vitest";
import {
  encodeBalanceProofPreimage,
  signBalanceProof,
  decodeBalanceProof,
  verifyBalanceProof,
  TICKET_SIZE,
  PREIMAGE_SIZE,
} from "../src/channel/ticket";
import { Quanta } from "../src/core/quanta";
import { Keypair } from "../src/crypto/keypair";

const FIXED_PRIVATE = new Uint8Array(32).fill(7);
const keypair = Keypair.fromPrivateKey(FIXED_PRIVATE);
const channelId = 42n;
const nonce = 1n;
const transferred = Quanta.fromAur("100.5");

describe("SES0 & SES1: Zero-Float & Exact 128-Byte Codec", () => {
  it("encodeBalanceProofPreimage menghasilkan tepat 64 byte", () => {
    const preimage = encodeBalanceProofPreimage(
      channelId,
      nonce,
      transferred,
      keypair.publicKey,
    );
    expect(preimage.length).toBe(PREIMAGE_SIZE);
    expect(PREIMAGE_SIZE).toBe(64);
  });

  it("signBalanceProof menghasilkan tepat 128 byte", () => {
    const ticket = signBalanceProof(channelId, nonce, transferred, keypair);
    expect(ticket.length).toBe(TICKET_SIZE);
    expect(TICKET_SIZE).toBe(128);
  });

  it("roundtrip decode memulihkan nilai LE dengan benar", () => {
    const ticket = signBalanceProof(channelId, nonce, transferred, keypair);
    const decoded = decodeBalanceProof(ticket);
    expect(decoded.channelId).toBe(channelId);
    expect(decoded.nonce).toBe(nonce);
    expect(decoded.transferredAmount.toBigInt()).toBe(transferred.toBigInt());
    expect(decoded.senderPubkey).toEqual(keypair.publicKey);
    expect(decoded.signature.length).toBe(64);
  });

  it("menolak data kurang dari 128 byte", () => {
    const short = new Uint8Array(120);
    expect(() => decodeBalanceProof(short)).toThrowError();
  });

  it("monotonicity: nonce dan transferred amount selalu naik", () => {
    const t1 = signBalanceProof(channelId, 1n, Quanta.fromAur("10"), keypair);
    const t2 = signBalanceProof(channelId, 2n, Quanta.fromAur("20"), keypair);
    const d1 = decodeBalanceProof(t1);
    const d2 = decodeBalanceProof(t2);
    expect(d2.nonce > d1.nonce).toBe(true);
    expect(d2.transferredAmount.toBigInt() >= d1.transferredAmount.toBigInt()).toBe(true);
  });
});

describe("SES2: Rust Channel Conformance", () => {
  it("signature lolos verifikasi ed25519 atas 64 byte preimage", () => {
    const ticket = signBalanceProof(channelId, nonce, transferred, keypair);
    expect(verifyBalanceProof(ticket)).toBe(true);
  });

  it("verify menolak tiket dengan signature rusak", () => {
    const ticket = signBalanceProof(channelId, nonce, transferred, keypair);
    const tampered = Uint8Array.from(ticket);
    tampered[100] ^= 0xff;
    expect(verifyBalanceProof(tampered)).toBe(false);
  });

  it("verify menolak tiket dengan pubkey salah", () => {
    const ticket = signBalanceProof(channelId, nonce, transferred, keypair);
    const wrong = Uint8Array.from(ticket);
    wrong[32] ^= 0xff;
    expect(verifyBalanceProof(wrong)).toBe(false);
  });
});