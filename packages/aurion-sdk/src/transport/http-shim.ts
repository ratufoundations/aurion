import { blake3 } from "@noble/hashes/blake3";
import { bytesToHex } from "@noble/hashes/utils";

import type { AccountResponse, TxReceipt, AurionTransport } from "./transport.interface";
import { AurionSdkError } from "../errors";

export interface HttpTransportOptions {
  baseUrl: string;
  headers?: Record<string, string>;
  timeoutMs?: number;
}

function statusKey(chainId: string): string {
  return `aurion/${chainId}/status`;
}

function accountKey(chainId: string, accountId: string): string {
  return `aurion/${chainId}/account/${accountId}`;
}

function txSubmitKey(chainId: string): string {
  return `aurion/${chainId}/tx/submit`;
}

function timeoutSignal(timeoutMs: number): AbortSignal {
  return AbortSignal.timeout(timeoutMs);
}

function envelopeHash(envelope: Uint8Array): string {
  return bytesToHex(blake3(envelope));
}

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const copy = Uint8Array.from(bytes);
  return copy.buffer;
}

export class HttpTransport implements AurionTransport {
  readonly baseUrl: string;
  private readonly defaultHeaders: Record<string, string>;
  private readonly timeoutMs: number;

  constructor(options: HttpTransportOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, "");
    if (this.baseUrl.length === 0) {
      throw new AurionSdkError("TRANSPORT_ERROR", "baseUrl tidak boleh kosong");
    }
    this.defaultHeaders = { ...options.headers };
    this.timeoutMs = options.timeoutMs ?? 15_000;
  }

  private async request(url: string, init?: RequestInit): Promise<Response> {
    try {
      return await fetch(url, { ...init, signal: timeoutSignal(this.timeoutMs) });
    } catch (err) {
      throw new AurionSdkError(
        "TRANSPORT_ERROR",
        `Permintaan gagal ke ${url}: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  }

  private async parseJson<T>(response: Response, what: string): Promise<T> {
    if (!response.ok) {
      throw new AurionSdkError(
        "TRANSPORT_ERROR",
        `${what} gagal: HTTP ${response.status} ${response.statusText}`,
      );
    }
    const text = await response.text();
    try {
      return JSON.parse(text) as T;
    } catch {
      throw new AurionSdkError(`TRANSPORT_ERROR`, `${what} mengembalikan JSON tidak valid`);
    }
  }

  async getBalance(chainId: string, accountId: string): Promise<string> {
    const json = await this.getAccount(chainId, accountId);
    return json.balance;
  }

  async getAccount(chainId: string, accountId: string): Promise<AccountResponse> {
    const url = `${this.baseUrl}/${accountKey(chainId, accountId)}`;
    const response = await this.request(url, {
      method: "GET",
      headers: { Accept: "application/json", ...this.defaultHeaders },
    });
    const json = await this.parseJson<AccountResponse>(response, `Kueri akun ${accountId}`);
    if (typeof json.balance !== "string") {
      throw new AurionSdkError(
        "TRANSPORT_ERROR",
        "Respons akun tidak menyajikan 'balance' sebagai string desimal",
      );
    }
    return json;
  }

  async submitEnvelope(chainId: string, envelope: Uint8Array): Promise<TxReceipt> {
    const url = `${this.baseUrl}/${txSubmitKey(chainId)}`;
    const response = await this.request(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/octet-stream",
        Accept: "application/json",
        ...this.defaultHeaders,
      },
      body: toArrayBuffer(envelope),
    });
    const json = await this.parseJson<{ accepted?: boolean; message?: string }>(
      response,
      "Submit transaksi",
    );
    return {
      txHash: envelopeHash(envelope),
      accepted: json.accepted ?? response.ok,
      message: json.message ?? "",
    };
  }

  readonly statusPath = (chainId: string) => statusKey(chainId);
}