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

// Lower-level instruction builder. Callers must add the payer signer account
// to the returned instruction's keys before submitting. Use verifyDrandBeacon
// for full auto-wiring via Anchor IDL (T2.04).
export function createVerifyInstruction(options: {
  round: bigint;
  signature: Uint8Array;
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

  return new TransactionInstruction({
    keys: [
      { pubkey: configPda, isSigner: false, isWritable: false },
    ],
    programId,
    data,
  });
}
