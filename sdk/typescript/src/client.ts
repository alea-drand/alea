import * as anchor from "@coral-xyz/anchor";
import type { Idl } from "@coral-xyz/anchor";
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

// @ts-ignore — resolveJsonModule handles this
import idlJson from "./idl/alea_verifier.json" assert { type: "json" };

const idl = idlJson as Idl;

type BrowserWallet = {
  sendTransaction: unknown;
  publicKey: PublicKey;
  signTransaction: unknown;
  signAllTransactions: unknown;
};

type Signer = Keypair | BrowserWallet;

function isBrowserWallet(signer: Signer): signer is BrowserWallet {
  return "sendTransaction" in signer;
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

export async function verifyDrandBeacon(args: {
  connection: Connection;
  signer: Signer;
  round: bigint;
  signature: Uint8Array;
  programId?: PublicKey;
  computeUnits?: number;
}): Promise<Uint8Array> {
  const wallet = isBrowserWallet(args.signer)
    ? (args.signer as anchor.Wallet)
    : new anchor.Wallet(args.signer as Keypair);

  const provider = new anchor.AnchorProvider(args.connection, wallet, {
    commitment: "confirmed",
  });

  const programId = args.programId ?? DEVNET_PROGRAM_ID;
  const program = new anchor.Program(idl, programId, provider);
  const configPda = getConfigAddress(programId);

  const cuLimit = args.computeUnits ?? 900_000;
  const cuIx = ComputeBudgetProgram.setComputeUnitLimit({ units: cuLimit });

  const tx = await (program.methods as any)
    .verify(new anchor.BN(args.round.toString()), Array.from(args.signature))
    .accounts({ config: configPda, payer: wallet.publicKey })
    .preInstructions([cuIx])
    .rpc({ commitment: "confirmed", skipPreflight: false });

  // Retry getTransaction — Helius indexer lags 2-5s post-send per learning note
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
    throw new Error(`Transaction failed on-chain: ${JSON.stringify(info.meta.err)}`);
  }

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
  signer: Signer;
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
