import { describe, expect, it } from "vitest";
import { ChannelClientSession, DepositExceededError } from "../src/channel/session";
import { Quanta } from "../src/core/quanta";
import { Keypair } from "../src/crypto/keypair";
import { TICKET_SIZE } from "../src/channel/ticket";

const FIXED_PRIVATE = new Uint8Array(32).fill(7);
const keypair = Keypair.fromPrivateKey(FIXED_PRIVATE);
const channelId = 42n;
const deposit = Quanta.fromAur("1000");

function makeSession(): ChannelClientSession {
  return new ChannelClientSession(
    {
      channelId,
      deposit,
      senderPubkey: keypair.publicKey,
      receiverPubkey: new Uint8Array(32).fill(9),
    },
    keypair,
  );
}

describe("SES3: Deposit Boundary Safety", () => {
  it("menolak tiket jika cumulative melebihi deposit", () => {
    const session = makeSession();
    session.createTick(Quanta.fromAur("600"));
    expect(() => session.createTick(Quanta.fromAur("500"))).toThrow(
      DepositExceededError,
    );
  });

  it("menerima tiket jika cumulative tepat sama dengan deposit", () => {
    const session = makeSession();
    const ticket = session.createTick(Quanta.fromAur("1000"));
    expect(ticket.length).toBe(TICKET_SIZE);
    expect(session.transferredAmount.toBigInt()).toBe(deposit.toBigInt());
  });

  it("nonce selalu bertambah secara monotonik", () => {
    const session = makeSession();
    expect(session.lastNonce).toBe(0n);
    session.createTick(Quanta.fromAur("10"));
    expect(session.lastNonce).toBe(1n);
    session.createTick(Quanta.fromAur("20"));
    expect(session.lastNonce).toBe(2n);
  });

  it("getState mengembalikan snapshot read-only yang benar", () => {
    const session = makeSession();
    session.createTick(Quanta.fromAur("100"));
    const state = session.getState();
    expect(state.channelId).toBe(channelId);
    expect(state.nonce).toBe(1n);
    expect(state.transferredAmount.toBigInt()).toBe(
      Quanta.fromAur("100").toBigInt(),
    );
  });
});