// scripts/devnet-verify-loop.ts — Phase 3.3 + 3.4 + 3.6 devnet test harness.
//
// Live mode (default): poll drand API for each consecutive round as it's
// emitted, submit a verify tx to devnet, assert randomness = sha256(sig),
// capture CU. Designed to be run twice in this session — first with
// --count 1 (surface result to Aaron), then with --count 9 for the
// remaining rounds once Aaron approves.
//
// Failure mode (--failure-cases): submit three deliberately-bad inputs
// and assert the on-chain error codes match errors.rs.
//
// Usage:
//   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
//   ANCHOR_WALLET=$HOME/.config/solana/alea-deployer.json \
//   npx ts-node scripts/devnet-verify-loop.ts [--count N] [--start-offset N]
//
//   ANCHOR_PROVIDER_URL=... ANCHOR_WALLET=... \
//   npx ts-node scripts/devnet-verify-loop.ts --failure-cases
//
// Spec: build-spec/phases/phase-3-devnet.md §3.3-3.6
// Plan: /Users/aaron/.claude/plans/use-rag-to-get-unified-journal.md Phase F-H

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  PublicKey,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import { createHash } from "crypto";
import * as fs from "fs";
import * as path from "path";

import aleaIdl from "../target/idl/alea_verifier.json";

// ---------------------------------------------------------------------------
// evmnet chain parameters (must match initialize.ts / crypto/constants.rs)
// ---------------------------------------------------------------------------

const EVMNET_GENESIS = 1_727_521_075;
const EVMNET_PERIOD = 3;
const EVMNET_CHAIN_HASH_HEX =
  "04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3";
const DRAND_BASE = `https://api.drand.sh/${EVMNET_CHAIN_HASH_HEX}/public`;

// ROUND_1 signature/randomness — used by failure case 2 (wrong-round pairing).
const ROUND_1_SIG_HEX =
  "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346" +
  "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373";

// BN254 field prime (Fq). Used by failure case 3 to craft an off-curve signature.
// (2^254 + small adjustment — exact hex below is the canonical evmnet Fq modulus.)
const BN254_FQ_HEX =
  "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47";

// SDK default per T2.A — fits SVDW (~415K) + Anchor overhead + headroom.
const CU_LIMIT = 900_000;
const CU_LIMIT_IX = ComputeBudgetProgram.setComputeUnitLimit({ units: CU_LIMIT });

// ---------------------------------------------------------------------------
// CLI flags
// ---------------------------------------------------------------------------

interface Flags {
  failureCases: boolean;
  count: number;
  startOffset: number;
}

function parseFlags(): Flags {
  const args = process.argv.slice(2);
  const flags: Flags = { failureCases: false, count: 10, startOffset: 1 };
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--failure-cases") flags.failureCases = true;
    else if (a === "--count") flags.count = parseInt(args[++i], 10);
    else if (a === "--start-offset") flags.startOffset = parseInt(args[++i], 10);
  }
  return flags;
}

// ---------------------------------------------------------------------------
// Drand API
// ---------------------------------------------------------------------------

interface Beacon {
  round: number;
  signature: string;
  randomness: string;
}

async function fetchBeaconLive(round: number, maxWaitMs = 10_000): Promise<Beacon> {
  const f: any = (globalThis as any).fetch;
  const start = Date.now();
  let attempt = 0;
  while (Date.now() - start < maxWaitMs) {
    attempt++;
    const resp = await f(`${DRAND_BASE}/${round}`);
    if (resp.ok) {
      return (await resp.json()) as Beacon;
    }
    if (resp.status === 404) {
      // Round not published yet — wait 1s and retry (live mode).
      process.stdout.write(
        `    [round ${round}] not yet published (attempt ${attempt}, ${Math.round((Date.now() - start) / 1000)}s elapsed); waiting...\n`,
      );
      await sleep(1000);
      continue;
    }
    throw new Error(`drand fetch round ${round} failed: HTTP ${resp.status}`);
  }
  throw new Error(`drand round ${round} not published within ${maxWaitMs}ms`);
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// ---------------------------------------------------------------------------
// Explorer link
// ---------------------------------------------------------------------------

function explorerUrl(sig: string): string {
  return `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
}

// ---------------------------------------------------------------------------
// Live-round happy-path loop
// ---------------------------------------------------------------------------

interface RoundResult {
  round: number;
  signature: string;  // tx signature (not drand sig)
  cu: number;
  randomnessHex: string;
  explorer: string;
}

async function runLiveLoop(flags: Flags): Promise<RoundResult[]> {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const aleaProgram = new Program(aleaIdl as anchor.Idl, provider);
  const [configPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    aleaProgram.programId,
  );

  // Verify Config PDA exists — proves Phase E complete.
  const configInfo = await provider.connection.getAccountInfo(configPda);
  if (configInfo === null) {
    throw new Error(
      `Config PDA ${configPda.toBase58()} does not exist on devnet. ` +
        `Did you run Phase E (scripts/initialize.ts)?`,
    );
  }

  const currentRound = Math.floor((Date.now() / 1000 - EVMNET_GENESIS) / EVMNET_PERIOD);
  const firstRound = currentRound + flags.startOffset;
  const lastRound = firstRound + flags.count - 1;

  console.log(`\n[live-loop] Program:      ${aleaProgram.programId.toBase58()}`);
  console.log(`[live-loop] Config PDA:   ${configPda.toBase58()}`);
  console.log(`[live-loop] Endpoint:     ${provider.connection.rpcEndpoint}`);
  console.log(
    `[live-loop] Round range:  ${firstRound}..${lastRound} (count=${flags.count}, live mode)\n`,
  );

  const results: RoundResult[] = [];
  for (let round = firstRound; round <= lastRound; round++) {
    const beacon = await fetchBeaconLive(round);
    const sigBuf = Buffer.from(beacon.signature, "hex");
    if (sigBuf.length !== 64) {
      throw new Error(`round ${round} sig length ${sigBuf.length} != 64`);
    }

    const txSig = await aleaProgram.methods
      .verify(new BN(round), Array.from(sigBuf))
      .accounts({
        config: configPda,
        payer: provider.wallet.publicKey,
      })
      .preInstructions([CU_LIMIT_IX])
      .rpc({ commitment: "finalized", skipPreflight: true });

    const info = await provider.connection.getTransaction(txSig, {
      commitment: "finalized",
      maxSupportedTransactionVersion: 0,
    });
    if (info?.meta?.err !== null) {
      throw new Error(
        `round ${round} tx failed on-chain: ${JSON.stringify(info?.meta?.err)}`,
      );
    }

    // Verify randomness = sha256(signature_bytes) (ADR 0036 byte-for-byte).
    const returnData = (info!.meta as any).returnData;
    if (!returnData) throw new Error(`round ${round} no return data`);
    const [retB64] = returnData.data as [string, string];
    const gotRand = Buffer.from(retB64, "base64").toString("hex");
    const expectedRand = createHash("sha256").update(sigBuf).digest("hex");
    if (gotRand !== expectedRand) {
      throw new Error(
        `round ${round} randomness mismatch: sha256(sig)=${expectedRand} got=${gotRand}`,
      );
    }
    // Also sanity-check against drand's reported randomness.
    if (gotRand !== beacon.randomness) {
      throw new Error(
        `round ${round} drand-API randomness mismatch: drand=${beacon.randomness} got=${gotRand}`,
      );
    }

    const cu = info!.meta!.computeUnitsConsumed!;
    const exp = explorerUrl(txSig);
    console.log(
      `  [round ${round}] ok CU=${cu.toLocaleString()} randomness=0x${gotRand.slice(0, 16)}... explorer=${exp}`,
    );
    results.push({ round, signature: txSig, cu, randomnessHex: gotRand, explorer: exp });
  }

  return results;
}

// ---------------------------------------------------------------------------
// Phase G: CU stats
// ---------------------------------------------------------------------------

function computeStats(cuValues: number[]) {
  const sorted = [...cuValues].sort((a, b) => a - b);
  const n = cuValues.length;
  const sum = cuValues.reduce((a, b) => a + b, 0);
  const mean = sum / n;
  const variance = cuValues.reduce((a, b) => a + (b - mean) ** 2, 0) / n;
  const stddev = Math.sqrt(variance);
  const pct = (p: number) => sorted[Math.min(n - 1, Math.floor(n * p))];
  return {
    n,
    min: sorted[0],
    p50: pct(0.5),
    mean: Math.round(mean),
    p95: pct(0.95),
    max: sorted[n - 1],
    stddev: Math.round(stddev),
    variancePctOfMean: (stddev / mean) * 100,
  };
}

// ---------------------------------------------------------------------------
// Phase H: failure cases
// ---------------------------------------------------------------------------

function errCode(err: any): number | undefined {
  return (
    err?.error?.errorCode?.number ??
    err?.errorCode?.number ??
    err?.code
  );
}

interface FailureCaseResult {
  label: string;
  expectedCode: number;
  actualCode: number | string;
  signature?: string;
  explorer?: string;
  passed: boolean;
}

async function runFailureCases(): Promise<FailureCaseResult[]> {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const aleaProgram = new Program(aleaIdl as anchor.Idl, provider);
  const [configPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    aleaProgram.programId,
  );

  const results: FailureCaseResult[] = [];

  // Case 1: Round 0 → 6002 RoundZero
  const case1: FailureCaseResult = {
    label: "round_zero",
    expectedCode: 6002,
    actualCode: "none",
    passed: false,
  };
  try {
    await aleaProgram.methods
      .verify(new BN(0), Array.from(Buffer.alloc(64)))
      .accounts({ config: configPda, payer: provider.wallet.publicKey })
      .preInstructions([CU_LIMIT_IX])
      .rpc({ commitment: "finalized", skipPreflight: true });
    case1.actualCode = "NO_ERROR";
  } catch (err: any) {
    case1.actualCode = errCode(err) ?? "unknown";
    case1.signature = err?.signature;
    if (case1.signature) case1.explorer = explorerUrl(case1.signature);
  }
  case1.passed = case1.actualCode === 6002;
  console.log(
    `  [case 1: ${case1.label}] expected=6002 actual=${case1.actualCode} ${case1.passed ? "✓" : "✗"}`,
  );
  results.push(case1);

  // Case 2: Round-1 signature against round=2 → 6000 InvalidSignature (pairing mismatch)
  const case2: FailureCaseResult = {
    label: "wrong_round_pairing_mismatch",
    expectedCode: 6000,
    actualCode: "none",
    passed: false,
  };
  try {
    await aleaProgram.methods
      .verify(new BN(2), Array.from(Buffer.from(ROUND_1_SIG_HEX, "hex")))
      .accounts({ config: configPda, payer: provider.wallet.publicKey })
      .preInstructions([CU_LIMIT_IX])
      .rpc({ commitment: "finalized", skipPreflight: true });
    case2.actualCode = "NO_ERROR";
  } catch (err: any) {
    case2.actualCode = errCode(err) ?? "unknown";
    case2.signature = err?.signature;
    if (case2.signature) case2.explorer = explorerUrl(case2.signature);
  }
  case2.passed = case2.actualCode === 6000;
  console.log(
    `  [case 2: ${case2.label}] expected=6000 actual=${case2.actualCode} ${case2.passed ? "✓" : "✗"}`,
  );
  results.push(case2);

  // Case 3: x = Fq (field prime) → off-curve → 6001 InvalidG1Point
  // Construct sig with x bytes = BN254 field modulus (NOT a valid Fq element).
  const badSig = Buffer.concat([
    Buffer.from(BN254_FQ_HEX, "hex"), // x = p (invalid Fq)
    Buffer.from(ROUND_1_SIG_HEX, "hex").subarray(32, 64), // y = any valid-looking bytes
  ]);
  const case3: FailureCaseResult = {
    label: "off_curve_x_equals_fq",
    expectedCode: 6001,
    actualCode: "none",
    passed: false,
  };
  try {
    await aleaProgram.methods
      .verify(new BN(1), Array.from(badSig))
      .accounts({ config: configPda, payer: provider.wallet.publicKey })
      .preInstructions([CU_LIMIT_IX])
      .rpc({ commitment: "finalized", skipPreflight: true });
    case3.actualCode = "NO_ERROR";
  } catch (err: any) {
    case3.actualCode = errCode(err) ?? "unknown";
    case3.signature = err?.signature;
    if (case3.signature) case3.explorer = explorerUrl(case3.signature);
  }
  case3.passed = case3.actualCode === 6001;
  console.log(
    `  [case 3: ${case3.label}] expected=6001 actual=${case3.actualCode} ${case3.passed ? "✓" : "✗"}`,
  );
  results.push(case3);

  return results;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const flags = parseFlags();
  console.log(`[devnet-verify-loop] flags: ${JSON.stringify(flags)}`);

  if (flags.failureCases) {
    console.log("\n=== Phase H — failure cases ===\n");
    const results = await runFailureCases();
    const allPassed = results.every((r) => r.passed);
    const outPath = path.resolve(__dirname, "../validation-report-phase3-failures.json");
    fs.writeFileSync(outPath, JSON.stringify(results, null, 2));
    console.log(`\n  Results written to ${outPath}`);
    if (!allPassed) {
      console.error("\n  ✗ One or more failure cases did NOT produce the expected error code.");
      process.exit(1);
    }
    console.log("\n  ✓ All 3 failure cases produced expected error codes.");
    return;
  }

  console.log("\n=== Phase F+G — live verify loop ===\n");
  const results = await runLiveLoop(flags);

  if (results.length >= 2) {
    const stats = computeStats(results.map((r) => r.cu));
    console.log("\n  === CU distribution (devnet) ===");
    console.log(`    n:       ${stats.n}`);
    console.log(`    min:     ${stats.min.toLocaleString()}`);
    console.log(`    p50:     ${stats.p50.toLocaleString()}`);
    console.log(`    mean:    ${stats.mean.toLocaleString()}`);
    console.log(`    p95:     ${stats.p95.toLocaleString()}`);
    console.log(`    max:     ${stats.max.toLocaleString()}`);
    console.log(`    stddev:  ${stats.stddev.toLocaleString()}`);
    console.log(`    var%:    ${stats.variancePctOfMean.toFixed(2)}%`);
    console.log(`    localnet Wave G baseline: max=413,874, variance=0.53%`);
    const outPath = path.resolve(__dirname, "../validation-report-phase3-cu.json");
    fs.writeFileSync(outPath, JSON.stringify({ stats, results }, null, 2));
    console.log(`\n  Full distribution written to ${outPath}`);
  }

  console.log(`\n  ✓ ${results.length}/${results.length} rounds verified on devnet.`);
}

main().catch((err) => {
  console.error("[devnet-verify-loop] UNHANDLED:", err);
  process.exit(2);
});
