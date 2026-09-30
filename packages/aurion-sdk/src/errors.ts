export type AurionErrorCode =
  | "INVALID_QUANTA_STRING"
  | "QUANTA_UNDERFLOW"
  | "QUANTA_OVERFLOW"
  | "INVALID_AMOUNT"
  | "INVALID_FEE"
  | "INVALID_ADDRESS"
  | "INVALID_LENGTH"
  | "INVALID_KEY"
  | "INVALID_MNEMONIC"
  | "ENVELOPE_MISMATCH"
  | "TRANSPORT_ERROR";

export class AurionSdkError extends Error {
  readonly code: AurionErrorCode;

  constructor(code: AurionErrorCode, message: string) {
    super(`[${code}] ${message}`);
    this.name = "AurionSdkError";
    this.code = code;
  }
}

export function invalidQuantaString(detail: string): AurionSdkError {
  return new AurionSdkError("INVALID_QUANTA_STRING", detail);
}

export function invalidAmount(detail: string): AurionSdkError {
  return new AurionSdkError("INVALID_AMOUNT", detail);
}

export function invalidFee(detail: string): AurionSdkError {
  return new AurionSdkError("INVALID_FEE", detail);
}

export function invalidAddress(detail: string): AurionSdkError {
  return new AurionSdkError("INVALID_ADDRESS", detail);
}

export function invalidLength(expected: number, got: number, what: string): AurionSdkError {
  return new AurionSdkError(
    "INVALID_LENGTH",
    `${what} harus ${expected} byte, tetapi menerima ${got} byte`,
  );
}