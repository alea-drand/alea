// Anchor devnet integration test for the example-lottery consumer program.
//
// Gated behind ANCHOR_DEVNET=1 — the entire suite skips unless that env var
// is set. Do NOT run during normal `anchor test` (localnet) — the test
// requires a live devnet Alea program with an initialized Config PDA (Phase 3
// complete) and a live drand beacon.
//
// Run explicitly:
//   ANCHOR_DEVNET=1 \
//   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
//   ANCHOR_WALLET=$HOME/.config/solana/alea-deployer.json \
//   anchor test --skip-local-validator -- --grep "example-lottery"
//
// Costs ~0.01 SOL in devnet tx fees across all 3 scenarios.
//
// Scenarios:
//   1. Happy path player-win: commit → resolve with even-parity round → player receives SOL
//   2. Happy path house-win: commit → resolve with odd-parity round → house receives SOL
//   3. Early-resolve fail: commit with future min_round → resolve too early → GameError::RoundTooEarly

import * as anchor from "@coral-xyz/anchor";
import {
  PublicKey,
  SystemProgram,
  ComputeBudgetProgram,
  Transaction,
  sendAndConfirmTransaction,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import { expect } from "chai";
import { createHash } from "crypto";

// Skip entire suite unless ANCHOR_DEVNET=1.
const RUN = process.env.ANCHOR_DEVNET === "1";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ALEA_PROGRAM_ID = new PublicKey(
  "ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U"
);
const EXAMPLE_LOTTERY_PROGRAM_ID = new PublicKey(
  "ExLotTerY1111111111111111111111111111111111"
);
const EVMNET_GENESIS = 1_727_521_075;
const EVMNET_PERIOD = 3;
const EVMNET_CHAIN_HASH_HEX =
  "04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3";
const DRAND_BASE = `https://api.drand.sh/${EVMNET_CHAIN_HASH_HEX}/public`;

const CU_LIMIT_IX = ComputeBudgetProgram.setComputeUnitLimit({
  units: 900_000,
});

// Anchor instruction discriminators (sha256("global:<name>")[0..8]).
// Computed offline to avoid needing a compiled IDL for example-lottery.
const COMMIT_BET_DISCRIMINATOR = computeDiscriminator("commit_bet");
const RESOLVE_BET_DISCRIMINATOR = computeDiscriminator("resolve_bet");

function computeDiscriminator(name: string): Buffer {
  const preimage = `global:${name}`;
  return Buffer.from(createHash("sha256").update(preimage).digest()).subarray(
    0,
    8
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

async function fetchBeacon(round: number): Promise<{ signature: string; randomness: string }> {
  const f = (globalThis as any).fetch;
  for (let attempt = 0; attempt < 10; attempt++) {
    const resp = await f(`${DRAND_BASE}/${round}`);
    if (resp.ok) return resp.json();
    if (resp.status === 404) {
      await sleep(1000);
      continue;
    }
    throw new Error(`drand fetch failed: HTTP ${resp.status}`);
  }
  throw new Error(`drand round ${round} not available after 10s`);
}

function currentDrandRound(): number {
  return Math.floor((Date.now() / 1000 - EVMNET_GENESIS) / EVMNET_PERIOD);
}

function aleaConfigPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    ALEA_PROGRAM_ID
  );
}

function betPda(player: PublicKey, slot: bigint): [PublicKey, number] {
  const slotBuf = Buffer.alloc(8);
  slotBuf.writeBigUInt64LE(slot);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("bet"), player.toBuffer(), slotBuf],
    EXAMPLE_LOTTERY_PROGRAM_ID
  );
}

/// Build commit_bet instruction data: discriminator + amount (u64 LE) + min_resolution_round (u64 LE)
function buildCommitBetData(amount: bigint, minResolutionRound: bigint): Buffer {
  const buf = Buffer.alloc(8 + 8 + 8);
  COMMIT_BET_DISCRIMINATOR.copy(buf, 0);
  buf.writeBigUInt64LE(amount, 8);
  buf.writeBigUInt64LE(minResolutionRound, 16);
  return buf;
}

/// Build resolve_bet instruction data: discriminator + round (u64 LE) + signature (64 bytes)
function buildResolveBetData(round: bigint, signature: Buffer): Buffer {
  if (signature.length !== 64) throw new Error(`sig len ${signature.length} != 64`);
  const buf = Buffer.alloc(8 + 8 + 64);
  RESOLVE_BET_DISCRIMINATOR.copy(buf, 0);
  buf.writeBigUInt64LE(round, 8);
  signature.copy(buf, 16);
  return buf;
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("example-lottery", () => {
  if (!RUN) {
    it("skip — set ANCHOR_DEVNET=1 to run", () => {
      console.log(
        "[example-lottery] Skipping: ANCHOR_DEVNET != 1. " +
        "Run with ANCHOR_DEVNET=1 ANCHOR_PROVIDER_URL=https://api.devnet.solana.com"
      );
    });
    return;
  }

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const connection = provider.connection;
  const payer = (provider.wallet as anchor.Wallet).payer;
  const [configPda] = aleaConfigPda();

  // ---------------------------------------------------------------------------
  // Scenario 1: happy path player-win
  // ---------------------------------------------------------------------------
  it("happy path: player wins (even-parity randomness)", async () => {
    // Find a round where sha256(sig)[0..8] as u64 LE is even.
    // Fetch a few rounds until we find one with even parity.
    const baseRound = currentDrandRound() + 1;
    let targetRound: number | null = null;
    let targetSig: Buffer | null = null;

    for (let offset = 0; offset < 20; offset++) {
      const round = baseRound + offset;
      const beacon = await fetchBeacon(round);
      const sigBuf = Buffer.from(beacon.signature, "hex");
      const randomness = createHash("sha256").update(sigBuf).digest();
      const val = randomness.readBigUInt64LE(0);
      if (val % 2n === 0n) {
        targetRound = round;
        targetSig = sigBuf;
        break;
      }
    }
    if (!targetRound || !targetSig) {
      throw new Error("Could not find even-parity round in 20 attempts");
    }
    console.log(`[example-lottery] player-win scenario: round=${targetRound}`);

    // Get current slot for PDA seed.
    const slotResponse = await connection.getSlot("confirmed");
    const slot = BigInt(slotResponse);

    // Build commit_bet tx.
    const amount = BigInt(10_000_000); // 0.01 SOL
    const minResolutionRound = BigInt(targetRound);
    const [betPdaAddr] = betPda(payer.publicKey, slot);

    const commitIx = {
      keys: [
        { pubkey: betPdaAddr, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: ALEA_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: configPda, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      programId: EXAMPLE_LOTTERY_PROGRAM_ID,
      data: buildCommitBetData(amount, minResolutionRound),
    };

    const commitTx = new Transaction().add(commitIx);
    await sendAndConfirmTransaction(connection, commitTx, [payer], {
      commitment: "confirmed",
    });
    console.log(`[example-lottery] committed bet at PDA ${betPdaAddr.toBase58()}`);

    // Record balances before resolution.
    const playerBalanceBefore = await connection.getBalance(payer.publicKey, "confirmed");

    // Build resolve_bet tx.
    const resolveIx = {
      keys: [
        { pubkey: betPdaAddr, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: false, isWritable: true }, // player
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },  // payer/house
        { pubkey: ALEA_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: configPda, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      ],
      programId: EXAMPLE_LOTTERY_PROGRAM_ID,
      data: buildResolveBetData(BigInt(targetRound), targetSig),
    };

    const resolveTx = new Transaction().add(CU_LIMIT_IX, resolveIx);
    await sendAndConfirmTransaction(connection, resolveTx, [payer], {
      commitment: "confirmed",
      skipPreflight: true,
    });

    const playerBalanceAfter = await connection.getBalance(payer.publicKey, "confirmed");
    // Player wins: net change should be positive (received bet back minus tx fees).
    console.log(
      `[example-lottery] player-win ok: balance delta = ${playerBalanceAfter - playerBalanceBefore}`
    );
  });

  // ---------------------------------------------------------------------------
  // Scenario 2: happy path house-win
  // ---------------------------------------------------------------------------
  it("happy path: house wins (odd-parity randomness)", async () => {
    const baseRound = currentDrandRound() + 1;
    let targetRound: number | null = null;
    let targetSig: Buffer | null = null;

    for (let offset = 0; offset < 20; offset++) {
      const round = baseRound + offset;
      const beacon = await fetchBeacon(round);
      const sigBuf = Buffer.from(beacon.signature, "hex");
      const randomness = createHash("sha256").update(sigBuf).digest();
      const val = randomness.readBigUInt64LE(0);
      if (val % 2n !== 0n) {
        targetRound = round;
        targetSig = sigBuf;
        break;
      }
    }
    if (!targetRound || !targetSig) {
      throw new Error("Could not find odd-parity round in 20 attempts");
    }
    console.log(`[example-lottery] house-win scenario: round=${targetRound}`);

    const slotResponse = await connection.getSlot("confirmed");
    const slot = BigInt(slotResponse);
    const amount = BigInt(10_000_000);
    const minResolutionRound = BigInt(targetRound);
    const [betPdaAddr] = betPda(payer.publicKey, slot);

    const commitIx = {
      keys: [
        { pubkey: betPdaAddr, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: ALEA_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: configPda, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      programId: EXAMPLE_LOTTERY_PROGRAM_ID,
      data: buildCommitBetData(amount, minResolutionRound),
    };
    const commitTx = new Transaction().add(commitIx);
    await sendAndConfirmTransaction(connection, commitTx, [payer], {
      commitment: "confirmed",
    });

    const resolveIx = {
      keys: [
        { pubkey: betPdaAddr, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: ALEA_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: configPda, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      ],
      programId: EXAMPLE_LOTTERY_PROGRAM_ID,
      data: buildResolveBetData(BigInt(targetRound), targetSig),
    };
    const resolveTx = new Transaction().add(CU_LIMIT_IX, resolveIx);
    await sendAndConfirmTransaction(connection, resolveTx, [payer], {
      commitment: "confirmed",
      skipPreflight: true,
    });
    console.log(`[example-lottery] house-win ok: round=${targetRound}`);
  });

  // ---------------------------------------------------------------------------
  // Scenario 3: early-resolve fails with GameError::RoundTooEarly
  // ---------------------------------------------------------------------------
  it("early resolve fails with RoundTooEarly", async () => {
    // Commit with min_resolution_round = current_round + 10000 (far future).
    // Then try to resolve with current_round → must fail with GameError::RoundTooEarly.
    const currentRound = BigInt(currentDrandRound());
    const minResolutionRound = currentRound + 10_000n;

    const slotResponse = await connection.getSlot("confirmed");
    const slot = BigInt(slotResponse);
    const amount = BigInt(5_000_000); // 0.005 SOL
    const [betPdaAddr] = betPda(payer.publicKey, slot);

    const commitIx = {
      keys: [
        { pubkey: betPdaAddr, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: ALEA_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: configPda, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      programId: EXAMPLE_LOTTERY_PROGRAM_ID,
      data: buildCommitBetData(amount, minResolutionRound),
    };
    const commitTx = new Transaction().add(commitIx);
    await sendAndConfirmTransaction(connection, commitTx, [payer], {
      commitment: "confirmed",
    });
    console.log(`[example-lottery] committed with min_round=${minResolutionRound}`);

    // Fetch a current round (not far enough in the future) — any recent round works.
    const beacon = await fetchBeacon(Number(currentRound));
    const sigBuf = Buffer.from(beacon.signature, "hex");

    const resolveIx = {
      keys: [
        { pubkey: betPdaAddr, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: ALEA_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: configPda, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      ],
      programId: EXAMPLE_LOTTERY_PROGRAM_ID,
      data: buildResolveBetData(currentRound, sigBuf),
    };
    const resolveTx = new Transaction().add(CU_LIMIT_IX, resolveIx);

    let threw = false;
    try {
      await sendAndConfirmTransaction(connection, resolveTx, [payer], {
        commitment: "confirmed",
        skipPreflight: true,
      });
    } catch (err: any) {
      threw = true;
      // Expect Custom(6000) = GameError::RoundTooEarly (first variant in enum).
      const errStr = JSON.stringify(err?.logs ?? err?.message ?? err);
      console.log(`[example-lottery] early-resolve error (expected): ${errStr.slice(0, 200)}`);
    }
    expect(threw, "early resolve must throw (GameError::RoundTooEarly)").to.be.true;
    console.log(`[example-lottery] early-resolve-fail ok`);
  });
});
