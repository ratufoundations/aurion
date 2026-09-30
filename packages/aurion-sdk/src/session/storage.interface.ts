export interface SessionStorage {
  get(key: string): Promise<Uint8Array | null>;
  set(key: string, val: Uint8Array): Promise<void>;
  remove(key: string): Promise<void>;
}