import * as anchor from "@coral-xyz/anchor";
import type { Idl, Wallet, Program as AnchorProgram } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";
import { DEVNET_PROGRAM_ID } from "./constants.js";
import { fetchBeacon, getCurrentRound } from "./drand.js";
import { getConfigAddress } from "./instruction.js";
import { AleaError, ERRORS } from "./errors.js";

// Load IDL via readFileSync. Avoids the Node-version-incompatible
// `import ... assert { type: "json" }` / `with { type: "json" }` syntax
// split (Node 18 supports only `assert`; Node 21+ supports only `with`).
// The IDL is bundled at dist/idl/ and src/idl/ per package.json `files`.
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const idlPath = join(__dirname, "idl", "alea_verifier.json");
const idl = JSON.parse(readFileSync(idlPath, "utf-8")) as Idl;

// T2.07 — structural discriminant: WalletContextState/AnchorWallet have sendTransaction,
// Keypair does not. Works without importing wallet-adapter types at runtime.
function isBrowserWallet(signer: Keypair | Wallet): signer is Wallet {
  return "sendTransaction" in signer;
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// Extract an on-chain Solana custom-program error code from any of the error
// shapes thrown by web3.js / Anchor under skipPreflight=true. Mirrors the
// errCode() helper in scripts/devnet-verify-loop.ts. Returns undefined if no
// recognizable code is present.
function extractErrorCode(err: unknown): number | undefined {
  if (!err || typeof err !== "object") return undefined;
  const e = err as Record<string, any>;

  // Anchor-wrapped error
  const anchorCode =
    e["error"]?.errorCode?.number ??
    e["errorCode"]?.number ??
    (typeof e["code"] === "number" ? e["code"] : undefined);
  if (typeof anchorCode === "number") return anchorCode;

  // Raw Solana {InstructionError: [ixIdx, {Custom: N}]}
  const ie = e["InstructionError"];
  if (Array.isArray(ie) && ie.length === 2) {
    const inner = ie[1];
    if (inner && typeof inner.Custom === "number") return inner.Custom;
  }

  // web3.js SendTransactionError: logs are under either `logs` or `transactionLogs`
  const logs: string[] | undefined =
    (Array.isArray(e["logs"]) && e["logs"]) ||
    (Array.isArray(e["transactionLogs"]) && e["transactionLogs"]) ||
    undefined;
  if (logs) {
    for (const line of logs) {
      const mA = String(line).match(/Error Number: (\d+)/);
      if (mA && mA[1]) return parseInt(mA[1], 10);
      const mB = String(line).match(/custom program error: 0x([0-9a-fA-F]+)/);
      if (mB && mB[1]) return parseInt(mB[1], 16);
    }
  }

  return undefined;
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

  // Build the transaction via Anchor's IDL (discriminator + arg serialization)
  // but SEND it ourselves via connection.sendRawTransaction. This bypasses
  // Anchor 0.30.1's `.rpc()` error-wrapping path, which is broken against
  // @solana/web3.js ≥ 1.98: Anchor calls `new SendTransactionError(msg, logs)`
  // (old 2-arg positional signature) but web3.js 1.98 changed the constructor
  // to object destructuring `{action, signature, transactionMessage, logs}`.
  // The result is a thrown Error with message "Unknown action 'undefined'"
  // and all properties undefined — losing all error-code information.
  //
  // By building the tx + signing + sending ourselves, we read the on-chain
  // failure cleanly from `getTransaction().meta.err` (raw Solana
  // `{InstructionError: [ixIdx, {Custom: N}]}` shape) and map to AleaError.
  // T1.09: BN accepts string for bigint safety.
  const anchorTx: anchor.web3.Transaction = await (program.methods as any)
    .verify(new anchor.BN(args.round.toString()), Array.from(args.signature))
    .accounts({ config: configPda, payer: wallet.publicKey })
    .preInstructions([cuIx])
    .transaction();

  const { blockhash, lastValidBlockHeight } = await args.connection.getLatestBlockhash("confirmed");
  anchorTx.recentBlockhash = blockhash;
  anchorTx.feePayer = wallet.publicKey;
  const signedTx = await wallet.signTransaction(anchorTx);

  // skipPreflight: true is REQUIRED for Alea verify — pairing outpaces the
  // preflight blockhash window under high-CU load. See framework-gotchas
  // and scripts/devnet-verify-loop.ts (all verify call sites use true).
  const tx: string = await args.connection.sendRawTransaction(
    signedTx.serialize(),
    { skipPreflight: true, maxRetries: 3 },
  );

  // Wait for confirmation. Don't use confirmTransaction's promise because on
  // failure it throws with a similarly-broken shape; instead, poll
  // getTransaction until it returns (confirmed commitment) and then inspect
  // meta.err ourselves.
  await args.connection.confirmTransaction(
    { signature: tx, blockhash, lastValidBlockHeight },
    "confirmed",
  ).catch(() => {
    // confirmTransaction rejects on on-chain failure — we'll read meta.err
    // from the polled getTransaction below. Swallow here to unify the error
    // path through the meta.err extraction logic.
  });

  // Retry getTransaction — Helius indexer lags 2-5s post-send per
  // [[2026-04-18-helius-devnet-indexer-lag]].
  let info = null;
  for (let attempt = 0; attempt < 15; attempt++) {
    info = await args.connection.getTransaction(tx, {
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
    // Raw Solana {InstructionError: [ixIdx, {Custom: N}]} — extract and map.
    const raw = info.meta.err as any;
    const ie = raw?.InstructionError;
    if (Array.isArray(ie) && ie.length === 2) {
      const inner = ie[1] as any;
      if (typeof inner?.Custom === "number") {
        const code: number = inner.Custom;
        const msg = ERRORS[code] ?? `Unknown error code ${code}`;
        throw new AleaError(code, msg);
      }
    }
    // Fall back to log-scan for "Error Number: N" patterns.
    const code = extractErrorCode({ logs: info.meta.logMessages });
    if (typeof code === "number") {
      throw new AleaError(code, ERRORS[code] ?? `Unknown error code ${code}`);
    }
    throw new Error(`Transaction failed on-chain: ${JSON.stringify(raw)}`);
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
