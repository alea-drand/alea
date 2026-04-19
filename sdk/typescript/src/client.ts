import * as anchor from "@coral-xyz/anchor";
import type { Idl, Wallet, Program as AnchorProgram } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  Commitment,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import { DEVNET_PROGRAM_ID } from "./constants.js";
import { fetchBeacon, getCurrentRound } from "./drand.js";
import { getConfigAddress } from "./instruction.js";
import { AleaError } from "./errors.js";

// @ts-ignore
import idlJson from "./idl/alea_verifier.json" assert { type: "json" };

const idl = idlJson as Idl;

// T2.07 — structural discriminant: WalletContextState/AnchorWallet have sendTransaction,
// Keypair does not. Works without importing wallet-adapter types at runtime.
function isBrowserWallet(signer: Keypair | Wallet): signer is Wallet {
  return "sendTransaction" in signer;
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

export async function verifyDrandBeacon(args: {
  connection: Connection;
  signer: Keypair | Wallet;
  round: bigint;
  signature: Uint8Array;
  programId?: PublicKey;
  computeUnits?: number;
}): Promise<Uint8Array> {
  const wallet: Wallet = isBrowserWallet(args.signer)
    ? args.signer
    : new anchor.Wallet(args.signer as Keypair);

  const provider = new anchor.AnchorProvider(args.connection, wallet, {
    commitment: "confirmed",
  });

  const programId = args.programId ?? DEVNET_PROGRAM_ID;

  // Anchor 0.30 Program constructor: (idl, provider?, coder?)
  // The IDL embeds the address — override programId via the provider approach
  // by patching the IDL address field if different from devnet.
  let effectiveIdl = idl;
  if (programId.toBase58() !== (idl as any).address) {
    effectiveIdl = { ...idl, address: programId.toBase58() } as Idl;
  }
  const program = new anchor.Program(effectiveIdl, provider) as AnchorProgram<Idl>;

  const configPda = getConfigAddress(programId);

  const cuLimit = args.computeUnits ?? 900_000;
  const cuIx = ComputeBudgetProgram.setComputeUnitLimit({ units: cuLimit });

  // T1.09 — BN constructor accepts string for bigint safety
  const tx = await (program.methods as any)
    .verify(new anchor.BN(args.round.toString()), Array.from(args.signature))
    .accounts({ config: configPda, payer: wallet.publicKey })
    .preInstructions([cuIx])
    // skipPreflight: true is REQUIRED for Alea verify — pairing outpaces the
    // preflight blockhash window under high-CU load. See framework-gotchas
    // and scripts/devnet-verify-loop.ts (all 4 verify call sites use true).
    // Errors still surface via tx.meta.err below.
    .rpc({ commitment: "confirmed", skipPreflight: true });

  // Retry getTransaction — Helius indexer lags 2-5s post-send per
  // 2026-04-18-helius-devnet-indexer-lag learning note.
  let info = null;
  for (let attempt = 0; attempt < 15; attempt++) {
    info = await args.connection.getTransaction(tx as string, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (info) break;
    await sleep(1000);
  }

  if (!info) {
    throw new AleaError(
      6009,
      `ReturnDataMissing: getTransaction returned null for sig ${tx}`,
    );
  }
  if (info.meta?.err) {
    throw new Error(
      `Transaction failed on-chain: ${JSON.stringify(info.meta.err)}`,
    );
  }

  // T2.03 — extract return data (32-byte randomness) from tx metadata
  const returnData = (info.meta as any)?.returnData;
  if (!returnData?.data) {
    throw new AleaError(
      6009,
      "ReturnDataMissing: verify succeeded but return data absent",
    );
  }

  const [dataB64] = returnData.data as [string, string];
  const decoded = Buffer.from(dataB64, "base64");
  if (decoded.length < 32) {
    throw new AleaError(
      6009,
      `ReturnDataMissing: expected 32 bytes, got ${decoded.length}`,
    );
  }

  return new Uint8Array(decoded.slice(0, 32));
}

export async function getVerifiedRandomness(options: {
  connection: Connection;
  signer: Keypair | Wallet;
  programId?: PublicKey;
  commitment?: Commitment;
  round?: bigint;
  computeUnits?: number;
}): Promise<Uint8Array> {
  const round = options.round ?? getCurrentRound();
  const beacon = await fetchBeacon(round);

  return verifyDrandBeacon({
    connection: options.connection,
    signer: options.signer,
    round: beacon.round,
    signature: beacon.signature,
    programId: options.programId,
    computeUnits: options.computeUnits,
  });
}
