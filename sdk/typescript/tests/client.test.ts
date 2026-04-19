import { describe, it, expect, vi, afterEach } from "vitest";
import { Keypair, PublicKey } from "@solana/web3.js";
import { getConfigAddress } from "../src/instruction.js";
import { DEVNET_PROGRAM_ID } from "../src/constants.js";
import { hexToBytes, ROUND_1_SIGNATURE_HEX } from "./fixtures.js";

// Discriminator for verify from IDL: [133, 161, 141, 48, 120, 198, 88, 150]
const VERIFY_DISCRIMINATOR = Buffer.from([133, 161, 141, 48, 120, 198, 88, 150]);

afterEach(() => {
  vi.restoreAllMocks();
});

describe("getConfigAddress", () => {
  it("derives config PDA from devnet program ID", () => {
    const pda = getConfigAddress(DEVNET_PROGRAM_ID);
    expect(pda).toBeInstanceOf(PublicKey);
    // PDA should be deterministic
    expect(getConfigAddress(DEVNET_PROGRAM_ID).toBase58()).toBe(pda.toBase58());
  });

  it("derives different PDAs for different program IDs", () => {
    const pid1 = DEVNET_PROGRAM_ID;
    const pid2 = Keypair.generate().publicKey;
    expect(getConfigAddress(pid1).toBase58()).not.toBe(getConfigAddress(pid2).toBase58());
  });

  it("uses DEVNET_PROGRAM_ID by default", () => {
    const withExplicit = getConfigAddress(DEVNET_PROGRAM_ID);
    const withDefault = getConfigAddress();
    expect(withDefault.toBase58()).toBe(withExplicit.toBase58());
  });
});

describe("createVerifyInstruction", () => {
  it("uses verify discriminator [133, 161, 141, 48, 120, 198, 88, 150]", async () => {
    const { createVerifyInstruction } = await import("../src/instruction.js");
    const sig = hexToBytes(ROUND_1_SIGNATURE_HEX);
    const payer = Keypair.generate().publicKey;
    const ix = createVerifyInstruction({ round: 1n, signature: sig, payer });

    const discriminatorBytes = Buffer.from(ix.data).slice(0, 8);
    expect(discriminatorBytes).toEqual(VERIFY_DISCRIMINATOR);
  });

  it("encodes round as u64 LE", async () => {
    const { createVerifyInstruction } = await import("../src/instruction.js");
    const sig = hexToBytes(ROUND_1_SIGNATURE_HEX);
    const payer = Keypair.generate().publicKey;
    const ix = createVerifyInstruction({ round: 1n, signature: sig, payer });

    const roundBuf = Buffer.from(ix.data).slice(8, 16);
    expect(roundBuf.readBigUInt64LE()).toBe(1n);
  });

  it("includes signature bytes after round", async () => {
    const { createVerifyInstruction } = await import("../src/instruction.js");
    const sig = hexToBytes(ROUND_1_SIGNATURE_HEX);
    const payer = Keypair.generate().publicKey;
    const ix = createVerifyInstruction({ round: 1n, signature: sig, payer });

    const sigBytes = Buffer.from(ix.data).slice(16, 80);
    expect(sigBytes).toEqual(Buffer.from(sig));
  });

  it("uses config PDA as first account (not writable, not signer)", async () => {
    const { createVerifyInstruction } = await import("../src/instruction.js");
    const sig = hexToBytes(ROUND_1_SIGNATURE_HEX);
    const payer = Keypair.generate().publicKey;
    const ix = createVerifyInstruction({ round: 1n, signature: sig, payer });

    const configPda = getConfigAddress(DEVNET_PROGRAM_ID);
    const firstKey = ix.keys[0];
    expect(firstKey).toBeDefined();
    expect(firstKey!.pubkey.toBase58()).toBe(configPda.toBase58());
    expect(firstKey!.isSigner).toBe(false);
    expect(firstKey!.isWritable).toBe(false);
  });

  it("uses correct program ID", async () => {
    const { createVerifyInstruction } = await import("../src/instruction.js");
    const sig = hexToBytes(ROUND_1_SIGNATURE_HEX);
    const payer = Keypair.generate().publicKey;
    const ix = createVerifyInstruction({ round: 1n, signature: sig, payer });

    expect(ix.programId.toBase58()).toBe(DEVNET_PROGRAM_ID.toBase58());
  });

  it("includes payer as second account (signer + writable) — phase 4.5 A3", async () => {
    const { createVerifyInstruction } = await import("../src/instruction.js");
    const sig = hexToBytes(ROUND_1_SIGNATURE_HEX);
    const payer = Keypair.generate().publicKey;
    const ix = createVerifyInstruction({ round: 1n, signature: sig, payer });

    expect(ix.keys.length).toBe(2);
    const payerKey = ix.keys[1];
    expect(payerKey).toBeDefined();
    expect(payerKey!.pubkey.toBase58()).toBe(payer.toBase58());
    expect(payerKey!.isSigner).toBe(true);
    expect(payerKey!.isWritable).toBe(true);
  });
});

describe("MAINNET_PROGRAM_ID", () => {
  it("is an alias for DEVNET_PROGRAM_ID (cluster-agnostic design)", async () => {
    const { MAINNET_PROGRAM_ID, DEVNET_PROGRAM_ID } = await import("../src/constants.js");
    // Alea deploys to the same vanity ID on all clusters; the Connection
    // object determines which cluster's deployment the tx targets. The
    // two exports are distinct symbols for intent clarity in consumer
    // code, but point to the same bytes.
    expect(MAINNET_PROGRAM_ID.toBase58()).toBe(DEVNET_PROGRAM_ID.toBase58());
    expect(MAINNET_PROGRAM_ID.toBase58()).toBe(
      "ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U",
    );
  });
});
