import { describe, it, expect, vi, afterEach } from "vitest";
import { fetchBeacon } from "../src/drand.js";
import { DRAND_ENDPOINTS, DRAND_CHAIN_HASH } from "../src/constants.js";
import { AleaError } from "../src/errors.js";
import {
  ROUND_1_SIGNATURE_HEX,
  ROUND_1_EXPECTED_RANDOMNESS_HEX,
  hexToBytes,
} from "./fixtures.js";

afterEach(() => {
  vi.restoreAllMocks();
});

function mockFetchResponse(status: number, body?: object): Response {
  // Phase 4.5: mock must expose `headers.get()` for the new Content-Length
  // size-cap check (T2-05). Return a Headers-shaped stub.
  const headers = new Headers({});
  return {
    ok: status >= 200 && status < 300,
    status,
    headers,
    json: async () => body,
  } as unknown as Response;
}

const ROUND_1_BODY = {
  round: 1,
  signature: ROUND_1_SIGNATURE_HEX,
  randomness: ROUND_1_EXPECTED_RANDOMNESS_HEX,
};

describe("fetchBeacon", () => {
  it("fetches round 1 from first endpoint on success", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      mockFetchResponse(200, ROUND_1_BODY),
    );

    const beacon = await fetchBeacon(1n);

    expect(beacon.round).toBe(1n);
    expect(beacon.unverifiedRandomness).toBe(ROUND_1_EXPECTED_RANDOMNESS_HEX);
    expect(beacon.signature).toEqual(hexToBytes(ROUND_1_SIGNATURE_HEX));

    // Only one endpoint should be called on success
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    const calledUrl = (fetchSpy.mock.calls[0] as [string])[0];
    expect(calledUrl).toContain(DRAND_ENDPOINTS[0]);
    expect(calledUrl).toContain(DRAND_CHAIN_HASH);
    expect(calledUrl).toContain("/public/1");
  });

  it("falls back to second endpoint on first failure", async () => {
    let callCount = 0;
    vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
      callCount++;
      if (callCount === 1) {
        throw new Error("network error");
      }
      return mockFetchResponse(200, ROUND_1_BODY);
    });

    const beacon = await fetchBeacon(1n);
    expect(beacon.round).toBe(1n);
    expect(callCount).toBe(2);
  });

  it("walks all 5 endpoints in order before retrying", async () => {
    const calledUrls: string[] = [];
    let callCount = 0;
    vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
      calledUrls.push(url as string);
      callCount++;
      if (callCount <= 5) {
        throw new Error("network error");
      }
      return mockFetchResponse(200, ROUND_1_BODY);
    });

    const beacon = await fetchBeacon(1n);
    expect(beacon.round).toBe(1n);

    // First 5 calls should be the 5 different endpoints
    for (let i = 0; i < 5; i++) {
      expect(calledUrls[i]).toContain(DRAND_ENDPOINTS[i]);
    }
    // 6th call should be endpoint[0] again (second attempt)
    expect(calledUrls[5]).toContain(DRAND_ENDPOINTS[0]);
  });

  it("throws AleaError after all retries exhausted", async () => {
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("network error"));

    await expect(fetchBeacon(1n)).rejects.toBeInstanceOf(AleaError);
  });

  it("returns unverifiedRandomness as hex string (not bytes)", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      mockFetchResponse(200, ROUND_1_BODY),
    );

    const beacon = await fetchBeacon(1n);
    expect(typeof beacon.unverifiedRandomness).toBe("string");
    expect(beacon.unverifiedRandomness).toBe(ROUND_1_EXPECTED_RANDOMNESS_HEX);
  });

  it("returns round as bigint", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      mockFetchResponse(200, ROUND_1_BODY),
    );

    const beacon = await fetchBeacon(1n);
    expect(typeof beacon.round).toBe("bigint");
  });
});
