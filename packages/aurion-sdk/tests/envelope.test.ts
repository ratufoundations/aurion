import * as ed25519 from "@noble/ed25519";
import { describe, expect, it } from "vitest";
import {
  ENVELOPE_SIZE,
  SIGNING_PAYLOAD_SIZE,
  Quanta,
  Keypair,
  buildSigningPayload,
  encodeEnvelope,
  decodeEnvelope,
  normalizeAddress,
  bytesToHex,
  writeU128LE,
} from "../src";

const FIXED_PRIVATE = new Uint8Array(32).fill(7);
const sender = Keypair.fromPrivateKey(FIXED_PRIVATE);
const recipient = Keypair.fromPrivateKey(new Uint8Array(32).fill(9));

const params = {
  nonce: 7n,
  amount: 1_234_567_890_123_456_789n,
  fee: 250_000_000n,
  sender: sender.publicKey,
  recipient: recipient.publicKey,
};

describe("Envelope 168-byte kanonikal (SDK1)", () => {
  it("encodeEnvelope menghasilkan buffer tepat 168 byte", () => {
    const signed = sender.signEnvelope(params);
    const buf = encodeEnvelope(signed);
    expect(buf).toBeInstanceOf(Uint8Array);
    expect(buf.length).toBe(ENVELOPE_SIZE);
  });

  it("preimage tanda tangan tepat 104 byte", () => {
    const payload = buildSigningPayload(params);
    expect(payload.length).toBe(SIGNING_PAYLOAD_SIZE);
  });

  it("round-trip decode(encode) memulihkan seluruh field byte-for-byte", () => {
    const signed = sender.signEnvelope(params);
    const decoded = decodeEnvelope(encodeEnvelope(signed));
    expect(decoded.nonce).toBe(params.nonce);
    expect(decoded.amount).toBe(params.amount);
    expect(decoded.fee).toBe(params.fee);
    expect(decoded.sender).toEqual(params.sender);
    expect(decoded.recipient).toEqual(params.recipient);
    expect(decoded.signature).toEqual(signed.signature);
  });

  it("offset field kanonikal mengikuti urutan nonce|sender|recipient|amount|fee|sig", () => {
    const signed = sender.signEnvelope(params);
    const buf = encodeEnvelope(signed);
    expect(buf.slice(0, 8)).toEqual(new Uint8Array(8).fill(0).map((_, i) => (i === 0 ? 7 : 0)));
    expect(buf.slice(8, 40)).toEqual(params.sender);
    expect(buf.slice(40, 72)).toEqual(params.recipient);
    const probe = new Uint8Array(16);
    writeU128LE(probe, 0, params.amount);
    expect(buf.slice(72, 88)).toEqual(probe);
    expect(buf.slice(88, 104)).not.toEqual(probe);
    expect(buf.slice(104, 168)).toEqual(signed.signature);
  });

  it("decodeEnvelope menolak panjang selain 168 byte (fail-fast)", () => {
    const signed = sender.signEnvelope(params);
    const buf = encodeEnvelope(signed);
    for (const bad of [0, 1, 103, 104, 107, 167]) {
      expect(() => decodeEnvelope(buf.slice(0, bad))).toThrowError();
    }
  });

  it("tamper pada region amount membuat verifikasi tanda tangan gagal", () => {
    const buf = encodeEnvelope(sender.signEnvelope(params));
    const tampered = Uint8Array.from(buf);
    tampered[80] = tampered[80] === 0xff ? 0x00 : (tampered[80] ?? 0) + 1;
    const decoded = decodeEnvelope(tampered);
    const payload = buildSigningPayload(decoded);
    expect(ed25519.verify(decoded.signature, payload, sender.publicKey)).toBe(false);
  });

  it("signature 64-byte diverifikasi dengan preimage 104 byte pertama (SDK2)", () => {
    const signed = sender.signEnvelope(params);
    const buf = encodeEnvelope(signed);
    const preimage = buf.subarray(0, SIGNING_PAYLOAD_SIZE);
    expect(preimage.length).toBe(104);
    expect(ed25519.verify(buf.subarray(104, 168), preimage, sender.publicKey)).toBe(true);
  });

  it("normalizeAddress menerima hex 64-karakter dan Uint8Array 32-byte, menolak input jinxed", () => {
    const hex = bytesToHex(sender.publicKey);
    expect(normalizeAddress(hex, "sender")).toEqual(sender.publicKey);
    expect(normalizeAddress(sender.publicKey, "sender")).toEqual(sender.publicKey);
    expect(() => normalizeAddress("0xzz", "sender")).toThrowError();
    expect(() => normalizeAddress(hex.slice(0, 62), "sender")).toThrowError();
    expect(() => normalizeAddress(new Uint8Array(31), "sender")).toThrowError();
  });

  it("nilai moneter di envelope tetap u128 (presisi > 2^53)", () => {
    const signed = sender.signEnvelope(params);
    const decoded = decodeEnvelope(encodeEnvelope(signed));
    expect(decoded.amount).toBe(1_234_567_890_123_456_789n);
    expect(decoded.amount > 9_007_199_254_740_991n).toBe(true);
    expect(Quanta.fromBigInt(decoded.fee).toAur()).toBe("0.025");
  });
});