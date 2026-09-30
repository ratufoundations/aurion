import { Quanta } from "../core/quanta";
import { DEFAULT_MIN_FEE_QUANTA } from "../core/constants";
import { encodeEnvelope, normalizeAddress, bytesToHex } from "../codec/envelope";
import { Keypair } from "../crypto/keypair";
import { invalidAmount, invalidFee } from "../errors";
import type { AurionTransport, TxReceipt } from "../transport/transport.interface";
import type { BalanceParams, NonceProvider, TransferParams } from "./types";

export interface AccountInfo {
  address: string;
  balance: Quanta;
  nonce: bigint;
}

export interface AurionClientOptions {
  nonceProvider?: NonceProvider;
}

export class AurionClient {
  constructor(
    readonly transport: AurionTransport,
    readonly chainId: string,
    private readonly options: AurionClientOptions = {},
  ) {}

  async getBalance({ accountId }: BalanceParams): Promise<Quanta> {
    return Quanta.fromQuantaString(await this.transport.getBalance(this.chainId, accountId));
  }

  async getAccount(accountId: string): Promise<AccountInfo> {
    const raw = await this.transport.getAccount(this.chainId, accountId);
    return {
      address: raw.address,
      balance: Quanta.fromQuantaString(raw.balance),
      nonce: BigInt(raw.nonce),
    };
  }

  private async resolveNonce(signer: Keypair, explicit?: bigint): Promise<bigint> {
    if (explicit !== undefined) {
      if (explicit < 0n) throw invalidFee("nonce tidak boleh negatif");
      return explicit;
    }
    if (this.options.nonceProvider) {
      const next = await this.options.nonceProvider.getNonce(signer.publicKeyHex);
      if (next < 0n) throw invalidFee("nonce dari provider tidak boleh negatif");
      return next;
    }
    return 0n;
  }

  signTransfer({
    signer,
    recipient,
    amount,
    fee,
    nonce,
  }: TransferParams) {
    if (amount.isZero()) {
      throw invalidAmount("amount harus lebih besar dari 0 Quanta");
    }
    const actualFee = fee ?? Quanta.fromBigInt(DEFAULT_MIN_FEE_QUANTA);
    if (actualFee.isZero()) {
      throw invalidFee("fee harus lebih besar dari 0 Quanta (protokol menolak fee == 0)");
    }
    const unsigned = {
      nonce: nonce ?? 0n,
      amount: amount.toBigInt(),
      fee: actualFee.toBigInt(),
      sender: signer.publicKey,
      recipient: normalizeAddress(recipient, "recipient"),
    };
    return signer.signEnvelope(unsigned);
  }

  async transfer(params: TransferParams): Promise<TxReceipt> {
    const signer = params.signer;
    const nonce = await this.resolveNonce(signer, params.nonce);
    const signed = this.signTransfer({ ...params, nonce });
    const envelope = encodeEnvelope(signed);
    return this.transport.submitEnvelope(this.chainId, envelope);
  }
}