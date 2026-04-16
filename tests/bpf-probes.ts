import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ComputeBudgetProgram } from "@solana/web3.js";
import { expect } from "chai";
import * as fs from "fs";
import * as path from "path";

const ROOT = process.cwd();

describe("Phase 1.1 BPF Probes", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idl = JSON.parse(
    fs.readFileSync(path.join(ROOT, "target/idl/alea_verifier.json"), "utf8")
  );
  const program = new Program(idl, provider);

  const CU_LIMIT = ComputeBudgetProgram.setComputeUnitLimit({
    units: 1_400_000,
  });

  describe("Solution A: G1 Decompression Syscall as Sqrt", () => {
    it("tests G1 decompress for sqrt(x³+3)", async () => {
      const tx = await program.methods
        .probeSyscall()
        .accounts({ signer: provider.wallet.publicKey })
        .preInstructions([CU_LIMIT])
        .rpc({ commitment: "confirmed" });

      const txDetails = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });

      console.log("\n=== G1 Decompression Syscall Test ===");
      const logs = txDetails?.meta?.logMessages || [];
      for (const log of logs) {
        console.log(log);
      }

      expect(txDetails).to.not.be.null;
      expect(txDetails!.meta!.err).to.be.null;
    });
  });
});
