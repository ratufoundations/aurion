import {
  ENVELOPE_SIZE,
  PUBKEY_SIZE,
  SIGNATURE_SIZE,
  SIGNING_PAYLOAD_SIZE,
  U64_SIZE,
  U128_SIZE,
} from "../core/constants";
import {
  readU64LE,
  readU128LE,
  writeU64LE,
  writeU128LE,
  assertCapacity,
} from "./binary";
import { invalidAddress, invalidLength, invalidAmount, invalidFee } from "../errors";

export interface UnsignedEnvelope {
  nonce: bigint;
  amount: bigint;
  fee: bigint;
  sender: Uint8Array;
  recipient: Uint8Array;
}

export interface SignedEnvelope extends UnsignedEnvelope {
  signature: Uint8Array;
}

export interface DecodedEnvelope {
  nonce: bigint;
  amount: bigint;
  fee: bigint;
  sender: Uint8Array;
  recipient: Uint8Array;
  signature: Uint8Array;
}

const HEX_RE = /^(0x)?[0-9a-fA-F]{64}$/;

export function normalizeAddress(value: string | Uint8Array, what: string): Uint8Array {
  if (typeof value === "string") {
    if (!HEX_RE.test(value)) {
      throw invalidAddress(
        `${what} harus 32-byte hex (64 karakter hex), menerima '${value}'`,
      );
    }
    const clean = value.startsWith("0x") ? value.slice(2) : value;
    const bytes = new Uint8Array(PUBKEY_SIZE);
    for (let i = 0; i < PUBKEY_SIZE; i += 1) {
      bytes[i] = Number.parseInt(clean.slice(i * 2, i * 2 + 2) as string, 16);
    }
    return bytes;
  }
  if (value.length !== PUBKEY_SIZE) {
    throw invalidAddress(`${what} harus tepat ${PUBKEY_SIZE} byte`);
  }
  return Uint8Array.from(value);
}

export function bytesToHex(bytes: Uint8Array): string {
  let out = "";
  for (const b of bytes) {
    out += b.toString(16).padStart(2, "0");
  }
  return out;
}

export function validateEnvelopeField(value: bigint, kind: "amount" | "fee"): void {
  if (kind === "amount") {
    if (value < 0n) throw invalidAmount("amount tidak boleh negatif");
  } else if (value < 0n) {
    throw invalidFee("fee tidak boleh negatif");
  }
}

/** Preimage 104 byte yang ditandatangani: nonce | sender | recipient | amount | fee (LE, urutan kanonikal). */
export function buildSigningPayload(params: UnsignedEnvelope): Uint8Array {
  const buf = new Uint8Array(SIGNING_PAYLOAD_SIZE);
  const sender = normalizeAddress(params.sender, "sender");
  const recipient = normalizeAddress(params.recipient, "recipient");
  validateEnvelopeField(params.amount, "amount");
  validateEnvelopeField(params.fee, "fee");
  writeU64LE(buf, 0, params.nonce);
  buf.set(sender, U64_SIZE);
  buf.set(recipient, U64_SIZE + PUBKEY_SIZE);
  writeU128LE(buf, U64_SIZE + PUBKEY_SIZE * 2, params.amount);
  writeU128LE(buf, U64_SIZE + PUBKEY_SIZE * 2 + U128_SIZE, params.fee);
  return buf;
}

export function encodeEnvelope(params: SignedEnvelope): Uint8Array {
  const payload = buildSigningPayload(params);
  const signature = params.signature;
  if (signature.length !== SIGNATURE_SIZE) {
    throw invalidLength(SIGNATURE_SIZE, signature.length, "signature");
  }
  const buf = new Uint8Array(ENVELOPE_SIZE);
  buf.set(payload, 0);
  buf.set(signature, SIGNING_PAYLOAD_SIZE);
  return buf;
}

export function decodeEnvelope(buf: Uint8Array): DecodedEnvelope {
  if (buf.length !== ENVELOPE_SIZE) {
    throw invalidLength(ENVELOPE_SIZE, buf.length, "envelope");
  }
  assertCapacity(buf, 0, ENVELOPE_SIZE, "envelope");
  const signature = Uint8Array.from(buf.slice(SIGNING_PAYLOAD_SIZE, ENVELOPE_SIZE));
  return {
    nonce: readU64LE(buf, 0),
    sender: Uint8Array.from(buf.slice(U64_SIZE, U64_SIZE + PUBKEY_SIZE)),
    recipient: Uint8Array.from(buf.slice(U64_SIZE + PUBKEY_SIZE, U64_SIZE + PUBKEY_SIZE * 2)),
    amount: readU128LE(buf, U64_SIZE + PUBKEY_SIZE * 2),
    fee: readU128LE(buf, U64_SIZE + PUBKEY_SIZE * 2 + U128_SIZE),
    signature,
  };
}