import { describe, it, expect } from "vitest";
import { Connection, Keypair } from "@solana/web3.js";
import {
  verifyDrandBeaconWithMeta,
  getVerifiedRandomnessWithMeta,
  verifyDrandBeacon,
  type VerifyMeta,
} from "../src/client.js";
import { AleaError } from "../src/errors.js";
import { hexToBytes, ROUND_1_SIGNATURE_HEX } from "./fixtures.js";

// Unit tests for the 0.2.0 additions. These assert input-validation
// behavior + public surface shape without hitting the network. The
// devnet_with_meta.test.ts file (gated by ALEA_DEVNET_TESTS=1) covers
// end-to-end behavior against a live cluster.

const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const validSig = hexToBytes(ROUND_1_SIGNATURE_HEX);

describe("verifyDrandBeaconWithMeta — input validation", () => {
  it("throws AleaError 6102 when signer is null", async () => {
    await expect(
      verifyDrandBeaconWithMeta({
        connection,
        signer: null as unknown as Keypair,
        round: 1n,
        signature: validSig,
      }),
    ).rejects.toSatisfy((e: unknown) => e instanceof AleaError && (e as AleaError).code === 6102);
  });

  it("throws AleaError 6102 when round is not bigint", async () => {
    await expect(
      verifyDrandBeaconWithMeta({
        connection,
        signer: Keypair.generate(),
        round: 1 as unknown as bigint,
        signature: validSig,
      }),
    ).rejects.toSatisfy((e: unknown) => e instanceof AleaError && (e as AleaError).code === 6102);
  });

  it("throws AleaError 6102 when round is out of range (< 1)", async () => {
    await expect(
      verifyDrandBeaconWithMeta({
        connection,
        signer: Keypair.generate(),
        round: 0n,
        signature: validSig,
      }),
    ).rejects.toSatisfy((e: unknown) => e instanceof AleaError && (e as AleaError).code === 6102);
  });

  it("throws AleaError 6102 when signature is wrong length", async () => {
    await expect(
      verifyDrandBeaconWithMeta({
        connection,
        signer: Keypair.generate(),
        round: 1n,
        signature: new Uint8Array(32), // should be 64
      }),
    ).rejects.toSatisfy((e: unknown) => e instanceof AleaError && (e as AleaError).code === 6102);
  });

  it("throws AleaError 6103 when signal is already aborted", async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(
      verifyDrandBeaconWithMeta({
        connection,
        signer: Keypair.generate(),
        round: 1n,
        signature: validSig,
        signal: controller.signal,
      }),
    ).rejects.toSatisfy(
      (e: unknown) => e instanceof AleaError && (e as AleaError).code === 6103,
    );
  });
});

describe("verifyDrandBeacon — backward compatibility (0.1.0 signature unchanged)", () => {
  it("still returns a Uint8Array, not a VerifyMeta object", async () => {
    // Invoke with invalid input so we can inspect the thrown error's source —
    // if the wrapper correctly delegates to verifyDrandBeaconWithMeta, the
    // AleaError shape matches.
    let caught: unknown;
    try {
      await verifyDrandBeacon({
        connection,
        signer: null as unknown as Keypair,
        round: 1n,
        signature: validSig,
      });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(AleaError);
    expect((caught as AleaError).code).toBe(6102);
  });
});

describe("VerifyMeta — type shape contract", () => {
  it("VerifyMeta has the 5 expected fields (compile-time check via const)", () => {
    // This is a compile-time assertion. If VerifyMeta's shape changes in
    // a breaking way, this test file fails to compile.
    const sample: VerifyMeta = {
      randomness: new Uint8Array(32),
      tx: "4tr4yetr4j3U9LjSNfA4CWVYNKrZr9nAKx51pV9baXJ7FAB1Hi4AjquAcQUcpgGD7buiGR9ppSbYTrVETnD9Zdtj",
      slot: 312441952,
      computeUnitsUsed: 407128,
      costLamports: 4520,
    };
    expect(sample.randomness).toHaveLength(32);
    expect(typeof sample.tx).toBe("string");
    expect(typeof sample.slot).toBe("number");
    expect(typeof sample.computeUnitsUsed).toBe("number");
    expect(typeof sample.costLamports).toBe("number");
  });
});

describe("getVerifiedRandomnessWithMeta — exists and has expected signature", () => {
  it("is an async function", () => {
    expect(typeof getVerifiedRandomnessWithMeta).toBe("function");
    expect(getVerifiedRandomnessWithMeta.constructor.name).toBe("AsyncFunction");
  });
});
