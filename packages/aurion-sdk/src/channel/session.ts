import { Quanta } from "../core/quanta";
import { Keypair } from "../crypto/keypair";
import type { ChannelConfig, TicketState } from "./types";
import { signBalanceProof, TICKET_SIZE } from "./ticket";

export class DepositExceededError extends Error {
  constructor(
    public readonly requested: bigint,
    public readonly deposit: bigint,
  ) {
    super(
      `Transfer ${requested} exceeds deposit ${deposit}`,
    );
    this.name = "DepositExceededError";
  }
}

export class ChannelClientSession {
  public readonly channelId: bigint;
  public readonly deposit: Quanta;
  public readonly senderPubkey: Uint8Array;
  public readonly receiverPubkey: Uint8Array;
  private readonly keypair: Keypair;
  private _lastNonce: bigint = 0n;
  private _transferredAmount: Quanta = Quanta.ZERO;

  constructor(config: ChannelConfig, keypair: Keypair) {
    this.channelId = config.channelId;
    this.deposit = config.deposit;
    this.senderPubkey = config.senderPubkey;
    this.receiverPubkey = config.receiverPubkey;
    this.keypair = keypair;
  }

  get lastNonce(): bigint {
    return this._lastNonce;
  }

  get transferredAmount(): Quanta {
    return this._transferredAmount;
  }

  getState(): TicketState {
    return {
      channelId: this.channelId,
      nonce: this._lastNonce,
      transferredAmount: this._transferredAmount,
      senderPubkey: new Uint8Array(this.senderPubkey),
    };
  }

  createTick(deltaAmount: Quanta): Uint8Array {
    const newTransferred = this._transferredAmount.add(deltaAmount);
    if (newTransferred.toBigInt() > this.deposit.toBigInt()) {
      throw new DepositExceededError(
        newTransferred.toBigInt(),
        this.deposit.toBigInt(),
      );
    }
    this._lastNonce += 1n;
    this._transferredAmount = newTransferred;
    return signBalanceProof(
      this.channelId,
      this._lastNonce,
      this._transferredAmount,
      this.keypair,
    );
  }
}