import { U64_SIZE, U128_SIZE } from "../core/constants";
import { invalidLength } from "../errors";

export const U64_MAX = 0xffff_ffff_ffff_ffffn;
export const U128_MAX = (1n << 128n) - 1n;

export function requireRange(value: bigint, max: bigint, what: string): void {
  if (value < 0n || value > max) {
    throw new RangeError(`${what} di luar rentang [0, ${max.toString()}] (menerima ${value.toString()})`);
  }
}

function writeUintLE(buf: Uint8Array, offset: number, value: bigint, width: number, max: bigint): void {
  requireRange(value, max, `nilai u${width * 8}`);
  let v = value;
  for (let i = 0; i < width; i += 1) {
    const at = offset + i;
    if (at >= buf.length) throw new RangeError("penulisan melewati batas buffer");
    buf[at] = Number(v & 0xffn);
    v >>= 8n;
  }
}

function writeUintBE(buf: Uint8Array, offset: number, value: bigint, width: number, max: bigint): void {
  requireRange(value, max, `nilai u${width * 8}`);
  let v = value;
  for (let i = width - 1; i >= 0; i -= 1) {
    const at = offset + i;
    if (at >= buf.length) throw new RangeError("penulisan melewati batas buffer");
    buf[at] = Number(v & 0xffn);
    v >>= 8n;
  }
}

function readUintLE(buf: Uint8Array, offset: number, width: number): bigint {
  let value = 0n;
  for (let i = width - 1; i >= 0; i -= 1) {
    value = (value << 8n) | BigInt(buf[offset + i] as number);
  }
  return value;
}

function readUintBE(buf: Uint8Array, offset: number, width: number): bigint {
  let value = 0n;
  for (let i = 0; i < width; i += 1) {
    value = (value << 8n) | BigInt(buf[offset + i] as number);
  }
  return value;
}

export function assertCapacity(buf: Uint8Array, offset: number, width: number, what: string): void {
  if (offset < 0 || offset + width > buf.length) {
    throw invalidLength(offset + width, buf.length, what);
  }
}

export function writeU64LE(buf: Uint8Array, offset: number, value: bigint): void {
  assertCapacity(buf, offset, U64_SIZE, "u64");
  writeUintLE(buf, offset, value, U64_SIZE, U64_MAX);
}

export function readU64LE(buf: Uint8Array, offset: number): bigint {
  assertCapacity(buf, offset, U64_SIZE, "u64");
  return readUintLE(buf, offset, U64_SIZE);
}

export function writeU128LE(buf: Uint8Array, offset: number, value: bigint): void {
  assertCapacity(buf, offset, U128_SIZE, "u128");
  writeUintLE(buf, offset, value, U128_SIZE, U128_MAX);
}

export function readU128LE(buf: Uint8Array, offset: number): bigint {
  assertCapacity(buf, offset, U128_SIZE, "u128");
  return readUintLE(buf, offset, U128_SIZE);
}

export function writeU64BE(buf: Uint8Array, offset: number, value: bigint): void {
  assertCapacity(buf, offset, U64_SIZE, "u64");
  writeUintBE(buf, offset, value, U64_SIZE, U64_MAX);
}

export function readU64BE(buf: Uint8Array, offset: number): bigint {
  assertCapacity(buf, offset, U64_SIZE, "u64");
  return readUintBE(buf, offset, U64_SIZE);
}

export function writeU128BE(buf: Uint8Array, offset: number, value: bigint): void {
  assertCapacity(buf, offset, U128_SIZE, "u128");
  writeUintBE(buf, offset, value, U128_SIZE, U128_MAX);
}

export function readU128BE(buf: Uint8Array, offset: number): bigint {
  assertCapacity(buf, offset, U128_SIZE, "u128");
  return readUintBE(buf, offset, U128_SIZE);
}