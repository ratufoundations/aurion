export {
  QUANTA_PER_AUR,
  AUR_DECIMALS,
  ENVELOPE_SIZE,
  SIGNING_PAYLOAD_SIZE,
  PUBKEY_SIZE,
  SIGNATURE_SIZE,
  ACCOUNT_ID_SIZE,
  ADDRESS_DOMAIN_TAG,
  DEFAULT_CHAIN_ID,
  DEFAULT_MIN_FEE_QUANTA,
} from "./core/constants";
export { Quanta, QuantaUnderflowError } from "./core/quanta";

export {
  readU64LE,
  readU128LE,
  writeU64LE,
  writeU128LE,
  readU64BE,
  readU128BE,
  writeU64BE,
  writeU128BE,
  U64_MAX,
  U128_MAX,
} from "./codec/binary";
export {
  buildSigningPayload,
  encodeEnvelope,
  decodeEnvelope,
  normalizeAddress,
  bytesToHex,
  type UnsignedEnvelope,
  type SignedEnvelope,
  type DecodedEnvelope,
} from "./codec/envelope";

export { Keypair, AurionMnemonicError } from "./crypto/keypair";

export { AurionClient, type AccountInfo, type AurionClientOptions } from "./client/aurion-client";
export type { TransferParams, BalanceParams, NonceProvider } from "./client/types";

export {
  HttpTransport,
  type HttpTransportOptions,
} from "./transport/http-shim";
export type { AurionTransport, TxReceipt, AccountResponse } from "./transport/transport.interface";

export { AurionSdkError, type AurionErrorCode } from "./errors";