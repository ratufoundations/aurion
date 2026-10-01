import { ClaimErrorResponse, ClaimRequest, ClaimResponse } from "../types";

export async function requestFaucetTokens(targetAddress: string): Promise<ClaimResponse> {
  const payload: ClaimRequest = {
    target_address: targetAddress.trim(),
  };

  try {
    const response = await fetch("/api/v1/claim", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(payload),
    });

    const data = await response.json();

    if (!response.ok || !data.success) {
      const errorData = data as Partial<ClaimErrorResponse>;
      return {
        success: false,
        error: errorData.error || `Error status: ${response.status}`,
        code: errorData.code || (response.status === 429 ? "COOLDOWN_ACTIVE" : "NODE_UNAVAILABLE"),
        retry_after_seconds: errorData.retry_after_seconds,
        cooldown_until: errorData.cooldown_until,
      };
    }

    return data as ClaimResponse;
  } catch (error: any) {
    // Network or client connection failure
    return {
      success: false,
      error: "Gagal terhubung ke faucet server. Periksa koneksi internet Anda atau status bootnode.",
      code: "NODE_UNAVAILABLE",
    };
  }
}
