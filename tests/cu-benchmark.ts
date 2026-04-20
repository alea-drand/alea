// Wave 11 — CU Benchmark. THE BREAKTHROUGH TEST.
//
// Fetches 50 consecutive live drand rounds from api.drand.sh/<chain>/public/<N>,
// runs each through alea.verify on localnet, records txInfo.meta.computeUnitsConsumed.
// Computes min/max/mean/stddev/p95/p99.
//
// Gate C (breakthrough confirmation):
//   AC-16: max < 1,000,000 CU
//   AC-17: variance < 20% of mean
// Policy: ANY max > 1M = hard stop, no workaround.

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  PublicKey,
  ComputeBudgetProgram,
  SystemProgram,
} from "@solana/web3.js";
import { expect } from "chai";

import aleaIdl from "../target/idl/alea_verifier.json";

const EVMNET_PUBKEY = Buffer.from(
  "07e1d1d335df83fa98462005690372c643340060d205306a9aa8106b6bd0b382" +
    "0557ec32c2ad488e4d4f6008f89a346f18492092ccc0d594610de2732c8b808f" +
    "0095685ae3a85ba243747b1b2f426049010f6b73a0cf1d389351d5aaaa1047f6" +
    "297d3a4f9749b33eb2d904c9d9ebf17224150ddd7abd7567a9bec6c74480ee0b",
  "hex",
);

const EVMNET_CHAIN_HASH_HEX =
  "04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3";
const EVMNET_CHAIN_HASH = Buffer.from(EVMNET_CHAIN_HASH_HEX, "hex");
const EVMNET_GENESIS = new BN(1727521075);
const EVMNET_PERIOD = new BN(3);

const CU_LIMIT_IX = ComputeBudgetProgram.setComputeUnitLimit({
  units: 1_400_000,
});

const NUM_ROUNDS = 50;
const DRAND_BASE = `https://api.drand.sh/${EVMNET_CHAIN_HASH_HEX}/public`;

interface Beacon {
  round: number;
  signature: string; // hex
  randomness: string; // hex
}

async function fetchBeacon(round: number): Promise<Beacon> {
  // Node 18+ has global fetch, but ts-mocha's TS lib config may not expose it
  // to the type checker. Cast global to any.
  const f: any = (globalThis as any).fetch;
  const resp = await f(`${DRAND_BASE}/${round}`);
  if (!resp.ok) {
    throw new Error(`drand fetch ${round} failed: ${resp.status}`);
  }
  return (await resp.json()) as Beacon;
}

function stats(xs: number[]) {
  const sorted = [...xs].sort((a, b) => a - b);
  const n = xs.length;
  const sum = xs.reduce((a, b) => a + b, 0);
  const mean = sum / n;
  const variance = xs.reduce((a, b) => a + (b - mean) ** 2, 0) / n;
  const stddev = Math.sqrt(variance);
  const pct = (p: number) => sorted[Math.min(n - 1, Math.floor(n * p))];
  return {
    n,
    min: sorted[0],
    max: sorted[n - 1],
    mean: Math.round(mean),
    stddev: Math.round(stddev),
    p50: pct(0.5),
    p95: pct(0.95),
    p99: pct(0.99),
    variancePctOfMean: (stddev / mean) * 100,
  };
}

describe("Wave 11 — CU Benchmark (BREAKTHROUGH GATE)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const aleaProgram = new Program(aleaIdl as anchor.Idl, provider);
  const [configPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    aleaProgram.programId,
  );

  before(async function () {
    this.timeout(60_000);
    // Init config PDA if not already (test file order independence).
    const info = await provider.connection.getAccountInfo(configPda);
    if (info === null) {
      await aleaProgram.methods
        .initialize(
          Array.from(EVMNET_PUBKEY),
          EVMNET_GENESIS,
          EVMNET_PERIOD,
          Array.from(EVMNET_CHAIN_HASH),
        )
        .accounts({
          config: configPda,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    }
  });

  it(`P1#13 verifies ${NUM_ROUNDS} live drand rounds, CU max < 1,000,000`, async function () {
    this.timeout(10 * 60_000); // 10 min ceiling

    // Pick a recent round range (skip a few from head to avoid race with next round)
    const currentRound = Math.floor(
      (Date.now() / 1000 - 1727521075) / 3,
    ) - 5;
    const rounds = Array.from(
      { length: NUM_ROUNDS },
      (_, i) => currentRound - NUM_ROUNDS + 1 + i,
    );

    console.log(
      `\n  Fetching ${rounds.length} beacons: rounds ${rounds[0]}..${rounds[rounds.length - 1]}`,
    );

    // Fetch beacons in parallel (bounded concurrency to avoid rate limits)
    const beacons: Beacon[] = [];
    const CONCURRENCY = 8;
    for (let i = 0; i < rounds.length; i += CONCURRENCY) {
      const batch = rounds
        .slice(i, i + CONCURRENCY)
        .map((r) => fetchBeacon(r));
      const results = await Promise.all(batch);
      beacons.push(...results);
    }
    console.log(`  Fetched ${beacons.length} beacons from drand API`);

    const cuUsed: number[] = [];
    let lastProgress = Date.now();

    for (const beacon of beacons) {
      const sigBuf = Buffer.from(beacon.signature, "hex");
      expect(sigBuf.length).to.equal(
        64,
        `round ${beacon.round} sig must be 64 bytes`,
      );

      const tx = await aleaProgram.methods
        .verify(new BN(beacon.round), Array.from(sigBuf))
        .accounts({
          config: configPda,
          payer: provider.wallet.publicKey,
        })
        .preInstructions([CU_LIMIT_IX])
        .rpc({ commitment: "confirmed", skipPreflight: true });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      expect(info?.meta?.err).to.equal(
        null,
        `round ${beacon.round} tx must succeed`,
      );

      // Verify randomness matches
      const [retB64] = (info!.meta as any).returnData.data as [string, string];
      const gotRand = Buffer.from(retB64, "base64").toString("hex");
      expect(gotRand).to.equal(
        beacon.randomness,
        `round ${beacon.round} randomness mismatch`,
      );

      const cu = info!.meta!.computeUnitsConsumed!;
      cuUsed.push(cu);

      if (Date.now() - lastProgress > 3000) {
        console.log(
          `    round ${beacon.round}: ${cu.toLocaleString()} CU (${cuUsed.length}/${beacons.length})`,
        );
        lastProgress = Date.now();
      }
    }

    const s = stats(cuUsed);
    console.log(`\n  === CU Distribution across ${s.n} rounds ===`);
    console.log(`    min:    ${s.min.toLocaleString()}`);
    console.log(`    p50:    ${s.p50.toLocaleString()}`);
    console.log(`    mean:   ${s.mean.toLocaleString()}`);
    console.log(`    p95:    ${s.p95.toLocaleString()}`);
    console.log(`    p99:    ${s.p99.toLocaleString()}`);
    console.log(`    max:    ${s.max.toLocaleString()}`);
    console.log(`    stddev: ${s.stddev.toLocaleString()}`);
    console.log(`    variance (%% of mean): ${s.variancePctOfMean.toFixed(2)}%%`);
    console.log("");

    // Export results for validation-report.md
    const reportPath = require("path").resolve(
      __dirname,
      "../validation-report-phase2-cu.json",
    );
    require("fs").writeFileSync(
      reportPath,
      JSON.stringify({ stats: s, cuUsed, rounds: beacons.map((b) => b.round) }, null, 2),
    );
    console.log(`  Full distribution written to ${reportPath}`);

    // Gate C assertions (policy: ANY overage = hard stop)
    expect(s.max, `AC-16: max CU must be < 1,000,000 (got ${s.max})`).to.be.lt(
      1_000_000,
    );

    // T2.G — AC-16b: max CU must be < 900K (SDK default budget boundary
    // per T2.A). The 1M gate above is a catastrophic-regression hard stop;
    // the 900K gate is the operational ceiling that matters for consumers
    // using the alea-sdk's auto-injected ComputeBudgetInstruction. Source:
    // P10-T2-06 (Sonnet test coverage).
    expect(
      s.max,
      `AC-16b: max CU must be < 900,000 (SDK default budget boundary; got ${s.max})`,
    ).to.be.lt(900_000);

    expect(
      s.variancePctOfMean,
      `AC-17: CU variance must be < 20%% of mean (got ${s.variancePctOfMean.toFixed(2)}%%)`,
    ).to.be.lt(20);
  });
});
