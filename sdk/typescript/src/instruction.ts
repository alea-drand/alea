import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import { DEVNET_PROGRAM_ID } from "./constants.js";

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
export function createVerifyInstruction(options: {
  round: bigint;
  signature: Uint8Array;
  /** Pays tx fee + must sign. Included in instruction keys as signer. */
  payer: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
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
