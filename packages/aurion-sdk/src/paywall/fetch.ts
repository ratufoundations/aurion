import { Quanta } from "../core/quanta";
import { ChannelClientSession } from "../channel/session";
import type { PaywallChallenge, PaywallOptions } from "./types";

function parseWwwAuthenticate(header: string): PaywallChallenge | null {
  const match = header.match(
    /Aurion-Channel\s+id=([0-9a-fA-F]+),\s*amount=(\d+)/,
  );
  if (!match) return null;
  return {
    channelId: BigInt(`0x${match[1]}`),
    amount: BigInt(match[2]),
  };
}

function toBase64(buf: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < buf.length; i++) {
    binary += String.fromCharCode(buf[i]!);
  }
  return btoa(binary);
}

export function createAurionFetch(
  session: ChannelClientSession,
  options: PaywallOptions = {},
): typeof fetch {
  const maxRetries = options.maxRetries ?? 1;

  return async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    let response = await fetch(input, init);

    for (let attempt = 0; attempt < maxRetries && response.status === 402; attempt++) {
      const header = response.headers.get("WWW-Authenticate");
      if (!header) break;

      const challenge = parseWwwAuthenticate(header);
      if (!challenge) break;

      const ticket = session.createTick(Quanta.fromBigInt(challenge.amount));
      options.onTicket?.(ticket);

      const headers = new Headers(init?.headers);
      headers.set("X-Aurion-Ticket", toBase64(ticket));

      response = await fetch(input, { ...init, headers });
    }

    return response;
  };
}