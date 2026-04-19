import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import { DEVNET_PROGRAM_ID } from "./constants.js";
import { AleaError } from "./errors.js";

const U64_MAX = 18_446_744_073_709_551_615n;

export function getConfigAddress(programId?: PublicKey): PublicKey {
  const pid = programId ?? DEVNET_PROGRAM_ID;
  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    pid,
  );
  return pda;
}

// Lower-level instruction builder. Returns a fully-keyed instruction including
// the payer as a signer key — the returned ix is ready to sign and submit.
// Use verifyDrandBeacon for full auto-wiring (tx construction + send + error
// extraction). Use this when you need raw control over transaction assembly
// (e.g., composing into a multi-instruction tx with your own compute-budget
// preflight or packing into a versioned transaction).
//
// Phase 4.5 T1-04: `round` is validated against the u64 domain before the
// Buffer.writeBigUInt64LE call (which throws an opaque Node RangeError on
// negative/oversize values). `signature` length is validated at 64 bytes
// to match the Anchor IDL's `[u8; 64]` serialization; oversize or short
// arrays produce a malformed instruction with confusing downstream errors.
export function createVerifyInstruction(options: {
  round: bigint;
  signature: Uint8Array;
  /** Pays tx fee + must sign. Included in instruction keys as signer. */
  payer: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
  // Input validation (SDK boundary — clear error messages beat opaque
  // web3.js / Node internals).
  if (typeof options.round !== "bigint") {
    throw new AleaError(
      6102,
      `InvalidInput: round must be bigint (got ${typeof options.round})`,
    );
  }
  if (options.round < 1n || options.round > U64_MAX) {
    throw new AleaError(
      6102,
      `InvalidInput: round must be in [1, 2^64-1] (got ${options.round.toString()})`,
    );
  }
  if (!(options.signature instanceof Uint8Array)) {
    throw new AleaError(
      6102,
      "InvalidInput: signature must be a Uint8Array",
    );
  }
  if (options.signature.length !== 64) {
    throw new AleaError(
      6102,
      `InvalidInput: signature must be exactly 64 bytes (got ${options.signature.length})`,
    );
  }
  if (!(options.payer instanceof PublicKey)) {
    throw new AleaError(
      6102,
      "InvalidInput: payer must be a PublicKey",
    );
  }

  const programId = options.programId ?? DEVNET_PROGRAM_ID;
  const configPda = getConfigAddress(programId);

  // Anchor verify discriminator from IDL: [133, 161, 141, 48, 120, 198, 88, 150]
  const discriminator = Buffer.from([133, 161, 141, 48, 120, 198, 88, 150]);

  // round u64 LE (8 bytes) + signature [u8; 64]
  const roundBuf = Buffer.alloc(8);
  roundBuf.writeBigUInt64LE(options.round);
  const sigBuf = Buffer.from(options.signature);

  const data = Buffer.concat([discriminator, roundBuf, sigBuf]);

  // Keys order matches Anchor's generated Verify accounts struct in
  // programs/alea-verifier: config first, payer second.
  return new TransactionInstruction({
    keys: [
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: options.payer, isSigner: true, isWritable: true },
    ],
    programId,
    data,
  });
}
