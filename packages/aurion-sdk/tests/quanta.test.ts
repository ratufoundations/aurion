import { describe, expect, it } from "vitest";
import {
  QUANTA_PER_AUR,
  Quanta,
  QuantaUnderflowError,
  AurionSdkError,
} from "../src";

function expectQuantaError(fn: () => unknown, code?: string): void {
  let thrown: unknown;
  try {
    fn();
  } catch (err) {
    thrown = err;
  }
  expect(thrown).toBeInstanceOf(Error);
  if (code !== undefined) {
    expect(thrown).toBeInstanceOf(AurionSdkError);
    expect((thrown as AurionSdkError).code).toBe(code);
  }
}

describe("Quanta: zero-float BigInt math", () => {
  it('"0.1" AUR menghasilkan tepat 1.000.000.000 Quanta', () => {
    expect(Quanta.fromAur("0.1").toBigInt()).toBe(1_000_000_000n);
  });

  it('"0.0000000001" AUR menghasilkan tepat 1 Quanta', () => {
    expect(Quanta.fromAur("0.0000000001").toBigInt()).toBe(1n);
  });

  it('"1" AUR menghasilkan QUANTA_PER_AUR', () => {
    expect(Quanta.fromAur("1").toBigInt()).toBe(QUANTA_PER_AUR);
  });

  it("konversi desimal penuh bersifat presisi dan deterministik", () => {
    const q = Quanta.fromAur("123456789.1234567890");
    expect(q.toBigInt()).toBe(123456789n * QUANTA_PER_AUR + 1_234_567_890n);
    expect(q.toAur()).toBe("123456789.123456789");
  });

  it("nilai melampaui 2^53-1 tidak kehilangan presisi", async () => {
    const huge = "9007199254740993.0000000001";
    const q = Quanta.fromAur(huge);
    expect(q.toAur()).toBe(huge);
    expect(q.toBigInt()).toBe(9_007_199_254_740_993n * QUANTA_PER_AUR + 1n);
  });

  it("toAur() menormalkan representasi tanpa trailing zero dan tanpa float", () => {
    expect(Quanta.fromAur("10").toAur()).toBe("10");
    expect(Quanta.fromAur("0").toAur()).toBe("0");
    expect(Quanta.fromAur("0.5").toAur()).toBe("0.5");
    expect(Quanta.fromAur("0.50").toAur()).toBe("0.5");
    expect(Quanta.fromQuantaString("5000000000").toAur()).toBe("0.5");
  });

  it("menolak float strings / notasi tak beraturan", () => {
    for (const bad of ["-1", "+1", "1e3", "1.5e3", "1.00000000001", "abc", "1.", ".5", "", " 1", "1 ", "NaN", "Infinity"]) {
      expectQuantaError(() => Quanta.fromAur(bad), "INVALID_QUANTA_STRING");
    }
  });

  it("menolak presisi melebihi 10 angka desimal (fail-fast tanpa pembulatan)", () => {
    expectQuantaError(() => Quanta.fromAur("0.12345678901"), "INVALID_QUANTA_STRING");
  });

  it("fromQuantaString menolak non-digit dan negatif", () => {
    expectQuantaError(() => Quanta.fromQuantaString("-1"), "INVALID_QUANTA_STRING");
    expectQuantaError(() => Quanta.fromQuantaString("12.5"), "INVALID_QUANTA_STRING");
    expect(Quanta.fromQuantaString("12345678901234567890").toBigInt()).toBe(12_345_678_901_234_567_890n);
  });

  it("arithmetic BigInt dengan proteksi underflow", () => {
    const a = Quanta.fromAur("1");
    const b = Quanta.fromAur("0.5");
    expect(a.add(b).toAur()).toBe("1.5");
    expect(a.sub(b).toAur()).toBe("0.5");
    expect(a.mul(3n).toAur()).toBe("3");
    expect(b.mul(2n).toAur()).toBe("1");
    expect(a.divFloor(2n).toAur()).toBe("0.5");
    expect(a.mulQuanta(b).isZero()).toBe(false);
    expect(() => Quanta.fromAur("0.1").sub(Quanta.fromAur("1"))).toThrow(QuantaUnderflowError);
  });

  it("isZero/equals/compare berperilaku benar", () => {
    expect(Quanta.ZERO.isZero()).toBe(true);
    expect(Quanta.fromAur("0.0000000000").isZero()).toBe(true);
    expect(Quanta.fromAur("0.5").equals(Quanta.fromAur("0.50"))).toBe(true);
    expect(Quanta.fromAur("2").compare(Quanta.fromAur("3"))).toBe(-1);
    expect(Quanta.fromAur("3").compare(Quanta.fromAur("3"))).toBe(0);
    expect(Quanta.fromAur("4").compare(Quanta.fromAur("3"))).toBe(1);
  });
});