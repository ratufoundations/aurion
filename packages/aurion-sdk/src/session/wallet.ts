import { Keypair } from "../crypto/keypair";
import { bytesToHex } from "../codec/envelope";
import type { SessionStorage } from "./storage.interface";
import { MemoryStorage } from "./storage-memory";

const STORAGE_KEY = "aurion-session-keypair";

export class SessionWallet {
  private readonly keypair: Keypair;
  private readonly storage: SessionStorage;

  private constructor(keypair: Keypair, storage: SessionStorage) {
    this.keypair = keypair;
    this.storage = storage;
  }

  get publicKey(): Uint8Array {
    return this.keypair.publicKey;
  }

  get address(): string {
    return bytesToHex(this.keypair.publicKey);
  }

  sign(data: Uint8Array): Uint8Array {
    return this.keypair.sign(data);
  }

  static async create(storage?: SessionStorage): Promise<SessionWallet> {
    const s = storage ?? new MemoryStorage();
    const keypair = Keypair.generate();
    await s.set(STORAGE_KEY, keypair.privateKey);
    return new SessionWallet(keypair, s);
  }

  static async loadOrCreate(storage?: SessionStorage): Promise<SessionWallet> {
    const s = storage ?? new MemoryStorage();
    const existing = await s.get(STORAGE_KEY);
    if (existing) {
      return new SessionWallet(Keypair.fromPrivateKey(existing), s);
    }
    return SessionWallet.create(s);
  }
}