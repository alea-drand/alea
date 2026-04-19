// scripts/initialize.ts — atomic deploy+init for Alea.
//
// T1.01 (Phase 2.5 Wave E) — the entire mitigation for FENDER-002 (front-
// run initialize → attacker captures permanent config.authority). Without
// this script, the deployer races the mempool manually between
// `solana program deploy` and any client-side `program.methods.initialize()`
// call. On mainnet this window is exploitable by any watcher that sees
// the deploy tx and submits a competing initialize with the correct
// evmnet constants (both public).
//
// This script builds a single versioned transaction containing:
//   1. ComputeBudgetInstruction.setComputeUnitLimit(900_000)
//   2. alea_verifier.initialize(EVMNET_PUBKEY, GENESIS, PERIOD, CHAIN_HASH)
//
// and sends it with `commitment: "finalized"`. Pre-flight aborts if
// Config PDA already exists (no accidental re-run); post-flight re-reads
// the account and asserts byte-equality on all 5 stored fields.
//
// Usage (from repo root):
//   anchor run initialize              # uses provider cluster from Anchor.toml
//   # OR directly:
//   npx ts-node scripts/initialize.ts
//
// Exit codes:
//   0  = Config PDA created, post-flight verified
//   1  = Config PDA already exists (idempotent skip; not an error)
//   2  = Pre-flight failure (RPC error, authority key mismatch)
//   3  = Initialize tx failed to confirm
//   4  = Post-flight byte-equality verification failed
//
// Sources: P06-T1-01 (Opus economic attacker), P08-T2-01 (Sonnet anchor
// model), FENDER-002 (Solana Fender static analysis).

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  PublicKey,
  ComputeBudgetProgram,
  Transaction,
  sendAndConfirmTransaction,
  SystemProgram,
} from "@solana/web3.js";

import aleaIdl from "../target/idl/alea_verifier.json";

// ---------------------------------------------------------------------------
// Canonical drand evmnet parameters
//
// These MUST match the on-chain `EXPECTED_EVMNET_*` constants in
// `programs/alea-verifier/src/crypto/constants.rs`. Any divergence causes
// the on-chain initialize handler to reject with AleaError codes 6007 /
// 6008 / 6010 / 6011 (T2.E byte-equality guards).
//
// Source: drand evmnet chain info (public, v2 scheme — see https://api.drand.sh/chains).
// ---------------------------------------------------------------------------

const EVMNET_PUBKEY = Buffer.from(
  "07e1d1d335df83fa98462005690372c643340060d205306a9aa8106b6bd0b382" +
    "0557ec32c2ad488e4d4f6008f89a346f18492092ccc0d594610de2732c8b808f" +
    "0095685ae3a85ba243747b1b2f426049010f6b73a0cf1d389351d5aaaa1047f6" +
    "297d3a4f9749b33eb2d904c9d9ebf17224150ddd7abd7567a9bec6c74480ee0b",
  "hex",
);

const EVMNET_CHAIN_HASH = Buffer.from(
  "04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3",
  "hex",
);

const EVMNET_GENESIS_TIME = new BN(1_727_521_075);
const EVMNET_PERIOD = new BN(3);

// Canonical Alea program ID. Anchor auto-derives from the IDL at runtime,
// so this is documentary / sanity-check only.
// new PublicKey("ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U")

// 900K CU = alea-sdk default. Fits SVDW (~415K observed) + Anchor overhead
// + consumer logic headroom (T2.A). Also used at verify-time by both SDKs.
const CU_LIMIT = 900_000;

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = new Program(aleaIdl as any, provider) as Program<any>;
  const authority = (provider.wallet as anchor.Wallet).payer;

  const [configPda, configBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId,
  );

  // Wave X+1 (Codex C HIGH, 2026-04-17) — ProgramData PDA under the
  // BPFLoaderUpgradeable program. The on-chain initialize handler
  // requires this account so it can assert signer == upgrade_authority,
  // closing the FENDER-002 deploy-to-init front-run window.
  const BPF_LOADER_UPGRADEABLE = new PublicKey(
    "BPFLoaderUpgradeab1e11111111111111111111111",
  );
  const [programDataPda] = PublicKey.findProgramAddressSync(
    [program.programId.toBuffer()],
    BPF_LOADER_UPGRADEABLE,
  );

  console.log(`[initialize] Program ID:     ${program.programId.toBase58()}`);
  console.log(`[initialize] Cluster:        ${provider.connection.rpcEndpoint}`);
  console.log(`[initialize] Authority:      ${authority.publicKey.toBase58()}`);
  console.log(`[initialize] Config PDA:     ${configPda.toBase58()} (bump ${configBump})`);
  console.log(`[initialize] ProgramData:    ${programDataPda.toBase58()}`);

  // -------------------------------------------------------------------------
  // Pre-flight: Config PDA existence check
  // -------------------------------------------------------------------------
  const configInfo = await provider.connection.getAccountInfo(configPda);
  if (configInfo !== null) {
    console.log(
      `[initialize] Config PDA already exists (${configInfo.data.length} bytes, owned by ${configInfo.owner.toBase58()}).`,
    );
    console.log("[initialize] Nothing to do. Exit 1 (idempotent skip).");
    process.exit(1);
  }

  // -------------------------------------------------------------------------
  // Pre-flight: authority key balance check
  // -------------------------------------------------------------------------
  const balance = await provider.connection.getBalance(authority.publicKey);
  const MIN_LAMPORTS = 0.01 * 1e9; // ≥ 0.01 SOL
  if (balance < MIN_LAMPORTS) {
    console.error(
      `[initialize] ERROR: authority has ${balance} lamports (need ≥ ${MIN_LAMPORTS} for rent).`,
    );
    process.exit(2);
  }

  // -------------------------------------------------------------------------
  // Build atomic tx: ComputeBudget + initialize
  // -------------------------------------------------------------------------
  const cuLimitIx = ComputeBudgetProgram.setComputeUnitLimit({
    units: CU_LIMIT,
  });

  const initializeIx = await program.methods
    .initialize(
      Array.from(EVMNET_PUBKEY),
      EVMNET_GENESIS_TIME,
      EVMNET_PERIOD,
      Array.from(EVMNET_CHAIN_HASH),
    )
    .accountsStrict({
      config: configPda,
      authority: authority.publicKey,
      programData: programDataPda,
      systemProgram: SystemProgram.programId,
    })
    .instruction();

  const tx = new Transaction().add(cuLimitIx, initializeIx);
  tx.feePayer = authority.publicKey;

  console.log("[initialize] Sending atomic tx (ComputeBudget + initialize)...");

  try {
    const sig = await sendAndConfirmTransaction(
      provider.connection,
      tx,
      [authority],
      { commitment: "finalized", skipPreflight: false },
    );
    console.log(`[initialize] tx confirmed: ${sig}`);
  } catch (err) {
    console.error("[initialize] ERROR: initialize tx failed:", err);
    process.exit(3);
  }

  // -------------------------------------------------------------------------
  // Post-flight: re-read Config PDA and byte-equality verify all fields
  // -------------------------------------------------------------------------
  const configAfter = await (program.account as any).config.fetch(configPda);

  const pubkeyMatch = Buffer.from(configAfter.pubkeyG2).equals(EVMNET_PUBKEY);
  const chainHashMatch = Buffer.from(configAfter.chainHash).equals(
    EVMNET_CHAIN_HASH,
  );
  const genesisMatch = configAfter.genesisTime.eq(EVMNET_GENESIS_TIME);
  const periodMatch = configAfter.period.eq(EVMNET_PERIOD);
  const authorityMatch = configAfter.authority.equals(authority.publicKey);
  const bumpMatch = configAfter.bump === configBump;

  if (
    !pubkeyMatch ||
    !chainHashMatch ||
    !genesisMatch ||
    !periodMatch ||
    !authorityMatch ||
    !bumpMatch
  ) {
    console.error("[initialize] ERROR: post-flight byte-equality FAILED:");
    console.error(`  pubkey_g2 match:    ${pubkeyMatch}`);
    console.error(`  chain_hash match:   ${chainHashMatch}`);
    console.error(`  genesis_time match: ${genesisMatch}`);
    console.error(`  period match:       ${periodMatch}`);
    console.error(`  authority match:    ${authorityMatch}`);
    console.error(`  bump match:         ${bumpMatch}`);
    process.exit(4);
  }

  console.log("[initialize] ✓ Post-flight verification passed.");
  console.log(`[initialize] ✓ Config PDA initialized at ${configPda.toBase58()}`);
  console.log(`[initialize] ✓ Authority: ${authority.publicKey.toBase58()}`);
  console.log("[initialize] Done. Ready for verify calls.");
}

main().catch((err) => {
  console.error("[initialize] UNHANDLED:", err);
  process.exit(2);
});
