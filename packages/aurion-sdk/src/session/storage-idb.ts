import type { SessionStorage } from "./storage.interface";

const DB_NAME = "aurion-session";
const STORE_NAME = "keys";
const DB_VERSION = 1;

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      req.result.createObjectStore(STORE_NAME);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export class IndexedDBStorage implements SessionStorage {
  private dbPromise: Promise<IDBDatabase>;

  constructor() {
    this.dbPromise = openDB();
  }

  async get(key: string): Promise<Uint8Array | null> {
    const db = await this.dbPromise;
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readonly");
      const req = tx.objectStore(STORE_NAME).get(key);
      req.onsuccess = () => {
        const val = req.result as ArrayBuffer | undefined;
        resolve(val ? new Uint8Array(val) : null);
      };
      req.onerror = () => reject(req.error);
    });
  }

  async set(key: string, val: Uint8Array): Promise<void> {
    const db = await this.dbPromise;
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readwrite");
      tx.objectStore(STORE_NAME).put(val.slice().buffer, key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  }

  async remove(key: string): Promise<void> {
    const db = await this.dbPromise;
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readwrite");
      tx.objectStore(STORE_NAME).delete(key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  }
}