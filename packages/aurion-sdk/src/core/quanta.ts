import { AUR_DECIMALS, QUANTA_PER_AUR } from "./constants";
import { invalidQuantaString } from "../errors";

const DECIMAL_PARTS = /^(\d+)(?:\.(\d+))?$/;

function quotRem(value: bigint, divisor: bigint): { quotient: bigint; remainder: bigint } {
  return { quotient: value / divisor, remainder: value % divisor };
}

export class Quanta {
  readonly value: bigint;

  private constructor(value: bigint) {
    if (value < 0n) {
      throw invalidQuantaString(
        `Nilai Quanta tidak boleh negatif (menerima ${value.toString()})`,
      );
    }
    this.value = value;
  }

  static ZERO = new Quanta(0n);

  static fromAur(val: string): Quanta {
    if (typeof val !== "string" || val.trim() !== val) {
      throw invalidQuantaString("Representasi AUR harus berupa string tanpa spasi tepi");
    }
    if (val.length === 0) {
      throw invalidQuantaString("Representasi AUR tidak boleh kosong");
    }
    if (val.startsWith("+") || val.startsWith("-")) {
      throw invalidQuantaString(
        `Representasi AUR tidak boleh bertanda (menerima '${val}')`,
      );
    }
    if (val.includes("e") || val.includes("E")) {
      throw invalidQuantaString("Notasi eksponensial (1eX) dilarang — gunakan string desimal murni");
    }
    const match = DECIMAL_PARTS.exec(val);
    if (match === null) {
      throw invalidQuantaString(`Representasi AUR tidak valid: '${val}'`);
    }
    const intPart = match[1] as string;
    const fracPart = match[2];
    if (fracPart !== undefined && fracPart.length > AUR_DECIMALS) {
      throw invalidQuantaString(
        `Presisi melebihi ${AUR_DECIMALS} angka desimal AUR ('${val}') — konversi akan kehilangan ketepatan`,
      );
    }
    const scaledFrac =
      fracPart === undefined ? 0n : BigInt(fracPart) * 10n ** BigInt(AUR_DECIMALS - fracPart.length);
    const whole = BigInt(intPart) * QUANTA_PER_AUR + scaledFrac;
    return new Quanta(whole);
  }

  static fromQuantaString(val: string): Quanta {
    if (typeof val !== "string" || !/^\d+$/.test(val)) {
      throw invalidQuantaString(`String Quanta tidak valid: '${String(val)}'`);
    }
    return new Quanta(BigInt(val));
  }

  static fromBigInt(val: bigint): Quanta {
    return new Quanta(val);
  }

  toAur(): string {
    if (this.value === 0n) return "0";
    const { quotient, remainder } = quotRem(this.value, QUANTA_PER_AUR);
    const frac = remainder.toString().padStart(AUR_DECIMALS, "0").replace(/0+$/, "");
    return frac.length === 0 ? quotient.toString() : `${quotient.toString()}.${frac}`;
  }

  toString(): string {
    return this.value.toString();
  }

  toBigInt(): bigint {
    return this.value;
  }

  equals(other: Quanta): boolean {
    return this.value === other.value;
  }

  isZero(): boolean {
    return this.value === 0n;
  }

  compare(other: Quanta): -1 | 0 | 1 {
    if (this.value < other.value) return -1;
    if (this.value > other.value) return 1;
    return 0;
  }

  add(other: Quanta): Quanta {
    return new Quanta(this.value + other.value);
  }

  sub(other: Quanta): Quanta {
    if (this.value < other.value) {
      throw new QuantaUnderflowError(other.value - this.value);
    }
    return new Quanta(this.value - other.value);
  }

  mul(scalar: bigint): Quanta {
    if (scalar < 0n) {
      throw invalidQuantaString(
        `Skalar perkalian tidak boleh negatif (menerima ${scalar.toString()})`,
      );
    }
    return new Quanta(this.value * scalar);
  }

  divFloor(divisor: bigint): Quanta {
    if (divisor <= 0n) {
      throw invalidQuantaString("Pembagian dengan nol atau negatif dilarang");
    }
    return new Quanta(this.value / divisor);
  }

  mulQuanta(other: Quanta): Quanta {
    return new Quanta(this.value * other.value);
  }
}

export class QuantaUnderflowError extends Error {
  constructor(delta: bigint) {
    super(`Saldo tidak cukup: kekurangan ${delta.toString()} Quanta`);
    this.name = "QuantaUnderflowError";
  }
}