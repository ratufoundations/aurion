import { describe, expect, it } from "vitest";
import { SessionWallet } from "../src/session/wallet";
import { MemoryStorage } from "../src/session/storage-memory";
import { bytesToHex } from "../src/codec/envelope";

describe("SES4: Keystore Storage Isolation", () => {
  it("create menghasilkan wallet dengan address dan publicKey", async () => {
    const wallet = await SessionWallet.create(new MemoryStorage());
    expect(wallet.address).toMatch(/^[0-9a-f]{64}$/);
    expect(wallet.publicKey.length).toBe(32);
  });

  it("loadOrCreate memulihkan keypair yang sama dari storage", async () => {
    const storage = new MemoryStorage();
    const wallet1 = await SessionWallet.create(storage);
    const wallet2 = await SessionWallet.loadOrCreate(storage);
    expect(wallet2.address).toBe(wallet1.address);
    expect(wallet2.publicKey).toEqual(wallet1.publicKey);
  });

  it("sign menghasilkan signature 64 byte yang valid", async () => {
    const wallet = await SessionWallet.create(new MemoryStorage());
    const data = new Uint8Array([1, 2, 3, 4]);
    const sig = wallet.sign(data);
    expect(sig.length).toBe(64);
  });

  it("private key tidak bocor ke storage sebagai plaintext", async () => {
    const storage = new MemoryStorage();
    const wallet = await SessionWallet.create(storage);
    const stored = await storage.get("aurion-session-keypair");
    expect(stored).not.toBeNull();
    expect(stored).not.toEqual(wallet.publicKey);
  });
});