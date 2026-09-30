import type { SessionStorage } from "./storage.interface";

export class MemoryStorage implements SessionStorage {
  private store = new Map<string, Uint8Array>();

  async get(key: string): Promise<Uint8Array | null> {
    return this.store.get(key) ?? null;
  }

  async set(key: string, val: Uint8Array): Promise<void> {
    this.store.set(key, new Uint8Array(val));
  }

  async remove(key: string): Promise<void> {
    this.store.delete(key);
  }
}