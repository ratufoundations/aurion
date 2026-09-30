import * as ed25519 from "@noble/ed25519";
import { etc } from "@noble/ed25519";
import { blake3 } from "@noble/hashes/blake3";
import { sha512 } from "@noble/hashes/sha512";
import { bytesToHex, hexToBytes, randomBytes, concatBytes } from "@noble/hashes/utils";
import { mnemonicToSeedSync, validateMnemonic } from "@scure/bip39";
import { wordlist } from "@scure/bip39/wordlists/english";

import { ACCOUNT_ID_SIZE, ADDRESS_DOMAIN_TAG, PUBKEY_SIZE, SIGNATURE_SIZE } from "../core/constants";
import { buildSigningPayload, type UnsignedEnvelope, type SignedEnvelope } from "../codec/envelope";
import { invalidLength } from "../errors";

etc.sha512Sync = (...messages: Uint8Array[]): Uint8Array => sha512(concatBytes(...messages));

function privFrom(input: Uint8Array, what: string): Uint8Array {
  if (input.length !== PUBKEY_SIZE) {
    throw invalidLength(PUBKEY_SIZE, input.length, what);
  }
  return Uint8Array.from(input);
}

export class Keypair {
  private readonly _privateKey: Uint8Array;
  readonly publicKey: Uint8Array;

  private constructor(privateKey: Uint8Array) {
    this._privateKey = privateKey;
    this.publicKey = ed25519.getPublicKey(privateKey);
  }

  static fromPrivateKey(privateKey: Uint8Array | string): Keypair {
    const input =
      typeof privateKey === "string" ? hexToBytes(privateKey) : Uint8Array.from(privateKey);
    return new Keypair(privFrom(input, "private key"));
  }

  static generate(): Keypair {
    return new Keypair(randomBytes(PUBKEY_SIZE));
  }

  static fromMnemonic(mnemonic: string, password = ""): Keypair {
    if (!validateMnemonic(mnemonic, wordlist)) {
      throw new AurionMnemonicError("Frasa mnemonik BIP-39 bahasa inggris tidak valid");
    }
    const seed = mnemonicToSeedSync(mnemonic, password);
    return new Keypair(privFrom(seed.slice(0, PUBKEY_SIZE), "seed (32 byte pertama)"));
  }

  get privateKeyHex(): string {
    return bytesToHex(this._privateKey);
  }

  get publicKeyHex(): string {
    return bytesToHex(this.publicKey);
  }

  get privateKey(): Uint8Array {
    return Uint8Array.from(this._privateKey);
  }

  sign(payload: Uint8Array): Uint8Array {
    return ed25519.sign(payload, this._privateKey);
  }

  signPayload(params: UnsignedEnvelope): Uint8Array {
    return this.sign(buildSigningPayload(params));
  }

  signEnvelope(params: UnsignedEnvelope): SignedEnvelope {
    return { ...params, signature: this.signPayload(params) };
  }

  verify(signature: Uint8Array, payload: Uint8Array): boolean {
    if (signature.length !== SIGNATURE_SIZE) {
      throw invalidLength(SIGNATURE_SIZE, signature.length, "signature");
    }
    return ed25519.verify(signature, payload, this.publicKey);
  }

  deriveAccountId(): Uint8Array {
    const domain = new TextEncoder().encode(ADDRESS_DOMAIN_TAG);
    const input = new Uint8Array(domain.length + this.publicKey.length);
    input.set(domain, 0);
    input.set(this.publicKey, domain.length);
    return blake3(input).slice(0, ACCOUNT_ID_SIZE);
  }

  get accountIdHex(): string {
    return bytesToHex(this.deriveAccountId());
  }
}

export class AurionMnemonicError extends Error {
  constructor(message: string) {
    super(`[INVALID_MNEMONIC] ${message}`);
    this.name = "AurionMnemonicError";
  }
}