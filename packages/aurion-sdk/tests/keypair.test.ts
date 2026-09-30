import * as ed25519 from "@noble/ed25519";
import { describe, expect, it } from "vitest";
import {
  PUBKEY_SIZE,
  SIGNATURE_SIZE,
  Keypair,
  AurionMnemonicError,
  buildSigningPayload,
  decodeEnvelope,
  encodeEnvelope,
  bytesToHex,
} from "../src";

const FIXED_PRIVATE = new Uint8Array(PUBKEY_SIZE).fill(42);
const MNEMONIC = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const payload104 = new Uint8Array(104).fill(3);

describe("Keypair Ed25519 (SDK2)", () => {
  it("fromPrivateKey menerima Uint8Array dan hex dengan hasil identik", () => {
    const a = Keypair.fromPrivateKey(FIXED_PRIVATE);
    const b = Keypair.fromPrivateKey(bytesToHex(FIXED_PRIVATE));
    expect(a.publicKey).toEqual(b.publicKey);
    expect(a.privateKeyHex).toBe(b.privateKeyHex);
    expect(a.publicKey.length).toBe(PUBKEY_SIZE);
  });

  it("generate() menghasilkan privat 32 byte / publik 32 byte", () => {
    const kp = Keypair.generate();
    expect(kp.privateKey.length).toBe(PUBKEY_SIZE);
    expect(kp.publicKey.length).toBe(PUBKEY_SIZE);
    expect(kp.privateKey).not.toEqual(new Uint8Array(PUBKEY_SIZE));
  });

  it("sign menghasilkan 64 byte; verify lulus untuk pesan benar dan gagal untuk pesan rusak", () => {
    const kp = Keypair.fromPrivateKey(FIXED_PRIVATE);
    const sig = kp.sign(payload104);
    expect(sig.length).toBe(SIGNATURE_SIZE);
    expect(kp.verify(sig, payload104)).toBe(true);

    const tampered = Uint8Array.from(payload104);
    tampered[40] = (tampered[40] ?? 0) ^ 0xff;
    expect(kp.verify(sig, tampered)).toBe(false);
  });

  it("verify menolak signature bukan 64 byte", () => {
    const kp = Keypair.fromPrivateKey(FIXED_PRIVATE);
    expect(() => kp.verify(new Uint8Array(63), payload104)).toThrowError();
  });

  it("tanda tangan SDK kompatibel dengan verifikator ed25519-dalek / noble (lokal)", () => {
    const kp = Keypair.generate();
    const sig = kp.sign(payload104);
    expect(ed25519.verify(sig, payload104, kp.publicKey)).toBe(true);
  });

  it("fromMnemonic deterministik dan menolak frasa tidak valid", () => {
    const a = Keypair.fromMnemonic(MNEMONIC);
    const b = Keypair.fromMnemonic(MNEMONIC);
    expect(a.privateKeyHex).toBe(b.privateKeyHex);
    expect(a.publicKey).toEqual(b.publicKey);
    expect(() => Keypair.fromMnemonic("tidak valid sama sekali")).toThrow(AurionMnemonicError);
  });

  it("deriveAccountId = BLAKE3(AURION_ADDR_CANONICAL_V1 || pubkey), deterministik 32 byte", () => {
    const kp = Keypair.generate();
    const id1 = kp.deriveAccountId();
    const id2 = kp.deriveAccountId();
    expect(id1.length).toBe(32);
    expect(id1).toEqual(id2);
    expect(kp.accountIdHex.length).toBe(64);
  });

  it("signEnvelope → encode → decode → verify preimage 104 byte (end-to-end SDK2)", () => {
    const sender = Keypair.generate();
    const recipient = Keypair.generate();
    const params = {
      nonce: 1n,
      amount: 100_000n,
      fee: 1n,
      sender: sender.publicKey,
      recipient: recipient.publicKey,
    };
    const signed = sender.signEnvelope(params);
    const buf = encodeEnvelope(signed);
    expect(buf.length).toBe(168);
    const decoded = decodeEnvelope(buf);
    const preimage = buildSigningPayload(decoded);
    expect(ed25519.verify(decoded.signature, preimage, sender.publicKey)).toBe(true);
  });
});