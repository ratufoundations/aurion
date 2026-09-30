import { afterEach, describe, expect, it, vi } from "vitest";
import { createAurionFetch } from "../src/paywall/fetch";
import { ChannelClientSession } from "../src/channel/session";
import { Quanta } from "../src/core/quanta";
import { Keypair } from "../src/crypto/keypair";
import { verifyBalanceProof, TICKET_SIZE } from "../src/channel/ticket";

const FIXED_PRIVATE = new Uint8Array(32).fill(7);
const keypair = Keypair.fromPrivateKey(FIXED_PRIVATE);
const channelId = 42n;
const deposit = Quanta.fromAur("10000");

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

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SES5: Paywall Interceptor Lifecycle", () => {
  it("menangani HTTP 402, menyematkan X-Aurion-Ticket, dan retry", async () => {
    const session = makeSession();
    const ticketHolder: Uint8Array[] = [];

    const mockFetch = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(null, {
          status: 402,
          headers: {
            "WWW-Authenticate": `Aurion-Channel id=${channelId.toString(16)}, amount=5000`,
          },
        }),
      )
      .mockResolvedValueOnce(new Response("OK", { status: 200 }));

    vi.stubGlobal("fetch", mockFetch);

    const aurionFetch = createAurionFetch(session, {
      onTicket: (t) => ticketHolder.push(t),
    });

    const response = await aurionFetch("https://example.com/api/data");
    expect(response.status).toBe(200);
    expect(mockFetch).toHaveBeenCalledTimes(2);

    expect(ticketHolder.length).toBe(1);
    const ticket = ticketHolder[0]!;
    expect(ticket.length).toBe(TICKET_SIZE);
    expect(verifyBalanceProof(ticket)).toBe(true);
  });

  it("meneruskan response non-402 tanpa modifikasi", async () => {
    const session = makeSession();
    const mockFetch = vi
      .fn()
      .mockResolvedValueOnce(new Response("OK", { status: 200 }));

    vi.stubGlobal("fetch", mockFetch);

    const aurionFetch = createAurionFetch(session);
    const response = await aurionFetch("https://example.com/api/data");
    expect(response.status).toBe(200);
    expect(mockFetch).toHaveBeenCalledTimes(1);
  });

  it("meneruskan 402 tanpa header WWW-Authenticate", async () => {
    const session = makeSession();
    const mockFetch = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 402 }));

    vi.stubGlobal("fetch", mockFetch);

    const aurionFetch = createAurionFetch(session);
    const response = await aurionFetch("https://example.com/api/data");
    expect(response.status).toBe(402);
    expect(mockFetch).toHaveBeenCalledTimes(1);
  });
});