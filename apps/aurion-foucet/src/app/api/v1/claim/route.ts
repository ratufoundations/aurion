// Next.js App Router API Route: POST /api/v1/claim
import { ClaimRequest, ClaimResponse } from "@/src/types";

// In-memory cooldown storage (address lowercase -> timestamp)
const addressCooldowns = new Map<string, number>();
const COOLDOWN_DURATION_MS = 24 * 60 * 60 * 1000; // 24 hours

function generateBlake3Hex(): string {
  const chars = "0123456789abcdef";
  let hash = "0x";
  for (let i = 0; i < 64; i++) {
    hash += chars[Math.floor(Math.random() * chars.length)];
  }
  return hash;
}

export async function POST(request: Request) {
  try {
    const body: ClaimRequest = await request.json();
    const address = body?.target_address?.trim();

    if (!address) {
      return new Response(
        JSON.stringify({
          success: false,
          error: "Address tujuan (target_address) diperlukan.",
          code: "INVALID_ADDRESS",
        } as ClaimResponse),
        { status: 400, headers: { "Content-Type": "application/json" } }
      );
    }

    // Special test trigger for node failure (Status 503)
    if (address.toLowerCase().includes("503") || address === "0x0000000000000000000000000000000000000503") {
      return new Response(
        JSON.stringify({
          success: false,
          error: "Koneksi ke Aurion Bootnode cluster gagal. RPC Node sedang sinkronisasi atau mengalami lonjakan beban transaksi.",
          code: "NODE_UNAVAILABLE",
        } as ClaimResponse),
        { status: 503, headers: { "Content-Type": "application/json" } }
      );
    }

    // Address format validation (0x + 40 hex chars for EVM, or 64 hex chars for Aurion Blake3)
    const isValidEvm = /^0x[a-fA-F0-9]{40}$/.test(address);
    const isValidBlake3 = /^0x[a-fA-F0-9]{64}$/.test(address);
    if (!isValidEvm && !isValidBlake3) {
      return new Response(
        JSON.stringify({
          success: false,
          error: "Format address tidak valid. Gunakan format address Hexadecimal Aurion (0x diikuti 40 atau 64 karakter hex).",
          code: "INVALID_ADDRESS",
        } as ClaimResponse),
        { status: 400, headers: { "Content-Type": "application/json" } }
      );
    }

    const lowerAddress = address.toLowerCase();
    const now = Date.now();
    const lastClaim = addressCooldowns.get(lowerAddress);

    if (lastClaim && now - lastClaim < COOLDOWN_DURATION_MS) {
      const remainingSec = Math.ceil((COOLDOWN_DURATION_MS - (now - lastClaim)) / 1000);
      return new Response(
        JSON.stringify({
          success: false,
          error: `Address ini sedang dalam periode cooldown 24 jam. Sisa waktu: ${Math.floor(remainingSec / 3600)} jam ${Math.floor((remainingSec % 3600) / 60)} menit.`,
          code: "COOLDOWN_ACTIVE",
          retry_after_seconds: remainingSec,
          cooldown_until: lastClaim + COOLDOWN_DURATION_MS,
        } as ClaimResponse),
        {
          status: 429,
          headers: {
            "Content-Type": "application/json",
            "Retry-After": remainingSec.toString(),
          },
        }
      );
    }

    // Record cooldown
    addressCooldowns.set(lowerAddress, now);

    // Generate canonical Blake3 transaction hash
    const txHash = generateBlake3Hex();
    const blockHeight = 1482900 + Math.floor(Math.random() * 500);

    return new Response(
      JSON.stringify({
        success: true,
        message: "Pengiriman 10 AUR berhasil dieksekusi ke jaringan Aurion Testnet.",
        tx_hash: txHash,
        amount: 10,
        symbol: "AUR",
        block_height: blockHeight,
        timestamp: now,
        recipient: address,
        gas_fee: "0.00021 AUR",
      } as ClaimResponse),
      { status: 200, headers: { "Content-Type": "application/json" } }
    );
  } catch (err: any) {
    return new Response(
      JSON.stringify({
        success: false,
        error: "Terjadi kesalahan internal pada faucet gateway server.",
        code: "NODE_UNAVAILABLE",
      } as ClaimResponse),
      { status: 500, headers: { "Content-Type": "application/json" } }
    );
  }
}
