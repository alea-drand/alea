import { describe, it, expect, beforeAll } from "vitest";
import { Connection, Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { readFileSync } from "fs";
import { homedir } from "os";
import { verifyDrandBeacon } from "../src/client.js";
import { DEVNET_PROGRAM_ID } from "../src/constants.js";
import { AleaError } from "../src/errors.js";
import {
  ROUND_1_SIGNATURE_HEX,
  ROUND_1_EXPECTED_RANDOMNESS_HEX,
  ROUND_9337227_SIGNATURE_HEX,
  ROUND_9337227_EXPECTED_RANDOMNESS_HEX,
  hexToBytes,
  bytesToHex,
} from "./fixtures.js";

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

  // Prefer ANCHOR_WALLET or the default deployer path — avoids airdrop faucet dependency
  const walletPath =
    process.env["ANCHOR_WALLET"] ??
    `${homedir()}/.config/solana/alea-deployer.json`;

  try {
    payer = loadKeypairFromFile(walletPath);
    const bal = await connection.getBalance(payer.publicKey);
    funded = bal >= 0.01 * LAMPORTS_PER_SOL;
    if (!funded) {
      console.warn(`[devnet] Wallet at ${walletPath} has < 0.01 SOL. Trying airdrop...`);
      const sig = await connection.requestAirdrop(payer.publicKey, 0.1 * LAMPORTS_PER_SOL);
      await connection.confirmTransaction(sig, "confirmed");
      const bal2 = await connection.getBalance(payer.publicKey);
      funded = bal2 >= 0.01 * LAMPORTS_PER_SOL;
    }
  } catch {
    // Wallet file not found — fall back to ephemeral + airdrop
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
    console.warn("[devnet] No funded wallet available. Skipping all devnet tests.");
  }
});

describe("devnet integration", () => {
  it("round-1 fixture verifies against live devnet", async () => {
    if (!ENABLED || !funded) {
      console.warn("[skip] ALEA_DEVNET_TESTS not set or airdrop failed");
      return;
    }

    const sig = hexToBytes(ROUND_1_SIGNATURE_HEX);
    const randomness = await verifyDrandBeacon({
      connection,
      signer: payer,
      round: 1n,
      signature: sig,
      programId: DEVNET_PROGRAM_ID,
    });

    expect(randomness).toHaveLength(32);
    expect(bytesToHex(randomness)).toBe(ROUND_1_EXPECTED_RANDOMNESS_HEX);
  });

  it("round-9337227 fixture verifies against live devnet", async () => {
    if (!ENABLED || !funded) {
      console.warn("[skip] ALEA_DEVNET_TESTS not set or airdrop failed");
      return;
    }

    const sig = hexToBytes(ROUND_9337227_SIGNATURE_HEX);
    const randomness = await verifyDrandBeacon({
      connection,
      signer: payer,
      round: 9337227n,
      signature: sig,
      programId: DEVNET_PROGRAM_ID,
    });

    expect(randomness).toHaveLength(32);
    expect(bytesToHex(randomness)).toBe(ROUND_9337227_EXPECTED_RANDOMNESS_HEX);
  });

  it("wrong-round failure returns AleaError code 6000 (InvalidSignature)", async () => {
    if (!ENABLED || !funded) {
      console.warn("[skip] ALEA_DEVNET_TESTS not set or airdrop failed");
      return;
    }

    // Submit round=1 with round-9337227's signature — pairing will fail
    const sig = hexToBytes(ROUND_9337227_SIGNATURE_HEX);

    let caught: unknown;
    try {
      await verifyDrandBeacon({
        connection,
        signer: payer,
        round: 1n,
        signature: sig,
        programId: DEVNET_PROGRAM_ID,
      });
    } catch (e) {
      caught = e;
    }

    expect(caught).toBeDefined();
    // Extract error code — Anchor-wrapped or raw InstructionError
    const code = extractCode(caught);
    expect(code).toBe(6000);
  });
});

function extractCode(err: unknown): number | undefined {
  if (!err || typeof err !== "object") return undefined;
  const e = err as Record<string, unknown>;
  const anchor =
    (e["error"] as any)?.errorCode?.number ??
    (e["errorCode"] as any)?.number ??
    (typeof e["code"] === "number" ? e["code"] : undefined);
  if (typeof anchor === "number") return anchor;
  const ie = (e["InstructionError"] as any[]) ?? undefined;
  if (Array.isArray(ie) && ie.length === 2) {
    const inner = ie[1] as Record<string, unknown>;
    if (inner && typeof inner["Custom"] === "number") return inner["Custom"];
  }
  // Try to parse from logs
  const logs = e["logs"] as string[] | undefined;
  if (Array.isArray(logs)) {
    for (const line of logs) {
      const m = String(line).match(/Error Number: (\d+)/);
      if (m) return parseInt(m[1]!, 10);
      const m2 = String(line).match(/custom program error: 0x([0-9a-fA-F]+)/);
      if (m2) return parseInt(m2[1]!, 16);
    }
  }
  return undefined;
}
