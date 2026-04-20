import { describe, it, expect, beforeAll } from "vitest";
import { Connection, Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { readFileSync } from "fs";
import { homedir } from "os";
import {
  verifyDrandBeaconWithMeta,
  getVerifiedRandomnessWithMeta,
} from "../src/client.js";
import { DEVNET_PROGRAM_ID } from "../src/constants.js";
import {
  ROUND_1_SIGNATURE_HEX,
  ROUND_1_EXPECTED_RANDOMNESS_HEX,
  hexToBytes,
  bytesToHex,
} from "./fixtures.js";

// Devnet integration tests for 0.2.0 meta-returning variants. Gated
// behind ALEA_DEVNET_TESTS=1 per the existing pattern in
// devnet_integration.test.ts. Uses the alea-deployer keypair when
// available, falls back to fresh airdrop otherwise.

const ENABLED = process.env["ALEA_DEVNET_TESTS"] === "1";

const connection = new Connection("https://api.devnet.solana.com", "confirmed");
let payer: Keypair;
let funded = false;

function loadKeypairFromFile(p: string): Keypair {
  const raw = JSON.parse(readFileSync(p, "utf-8")) as number[];
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

beforeAll(async () => {
  if (!ENABLED) return;

  const walletPath =
    process.env["ANCHOR_WALLET"] ??
    `${homedir()}/.config/solana/alea-deployer.json`;

  try {
    payer = loadKeypairFromFile(walletPath);
    const bal = await connection.getBalance(payer.publicKey);
    funded = bal >= 0.01 * LAMPORTS_PER_SOL;
    if (!funded) {
      const sig = await connection.requestAirdrop(payer.publicKey, 0.1 * LAMPORTS_PER_SOL);
      await connection.confirmTransaction(sig, "confirmed");
      const bal2 = await connection.getBalance(payer.publicKey);
      funded = bal2 >= 0.01 * LAMPORTS_PER_SOL;
    }
  } catch {
    payer = Keypair.generate();
    try {
      const sig = await connection.requestAirdrop(payer.publicKey, 0.1 * LAMPORTS_PER_SOL);
      await connection.confirmTransaction(sig, "confirmed");
      const bal = await connection.getBalance(payer.publicKey);
      funded = bal >= 0.01 * LAMPORTS_PER_SOL;
    } catch {
      funded = false;
    }
  }

  if (!funded) {
    console.warn("[devnet-with-meta] No funded wallet available. Skipping.");
  }
});

describe("devnet integration — meta variants", () => {
  it("verifyDrandBeaconWithMeta returns full meta shape on round-1 fixture", async () => {
    if (!ENABLED || !funded) {
      console.warn("[skip] ALEA_DEVNET_TESTS not set or airdrop failed");
      return;
    }

    const sig = hexToBytes(ROUND_1_SIGNATURE_HEX);
    const result = await verifyDrandBeaconWithMeta({
      connection,
      signer: payer,
      round: 1n,
      signature: sig,
      programId: DEVNET_PROGRAM_ID,
    });

    // Randomness matches the existing fixture
    expect(result.randomness).toHaveLength(32);
    expect(bytesToHex(result.randomness)).toBe(ROUND_1_EXPECTED_RANDOMNESS_HEX);

    // Tx sig is a base58 string of the expected length (Solana sigs are 64 bytes → ~87 chars base58)
    expect(typeof result.tx).toBe("string");
    expect(result.tx.length).toBeGreaterThan(80);
    expect(result.tx.length).toBeLessThan(100);

    // Slot is a positive integer from the confirmed tx
    expect(result.slot).toBeGreaterThan(0);
    expect(Number.isInteger(result.slot)).toBe(true);

    // CU is in the expected range for Alea verify (~400-500K)
    expect(result.computeUnitsUsed).toBeGreaterThan(0);
    expect(result.computeUnitsUsed).toBeLessThan(1_000_000);

    // Fee is positive lamports (typical 5000 base + CU cost)
    expect(result.costLamports).toBeGreaterThan(0);
  });

  it("getVerifiedRandomnessWithMeta returns round+signature+full meta", async () => {
    if (!ENABLED || !funded) {
      console.warn("[skip] ALEA_DEVNET_TESTS not set or airdrop failed");
      return;
    }

    const result = await getVerifiedRandomnessWithMeta({
      connection,
      signer: payer,
      programId: DEVNET_PROGRAM_ID,
    });

    // drand-level fields populated by fetchBeacon
    expect(typeof result.round).toBe("bigint");
    expect(result.round).toBeGreaterThan(0n);
    expect(result.signature).toBeInstanceOf(Uint8Array);
    expect(result.signature).toHaveLength(64);

    // on-chain meta populated by verifyDrandBeaconWithMeta
    expect(result.randomness).toHaveLength(32);
    expect(typeof result.tx).toBe("string");
    expect(result.slot).toBeGreaterThan(0);
    expect(result.computeUnitsUsed).toBeGreaterThan(0);
    expect(result.costLamports).toBeGreaterThan(0);
  }, 45_000); // drand fetch + verify can take ~10-15s on devnet
});
