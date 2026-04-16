import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ComputeBudgetProgram } from "@solana/web3.js";
import { expect } from "chai";
import * as fs from "fs";
import * as path from "path";

const ROOT = process.cwd();

const chainInfo = JSON.parse(
  fs.readFileSync(
    path.join(ROOT, "build-spec/testing/fixtures/chain-info.json"),
    "utf8"
  )
);
const g2NonSubgroup = JSON.parse(
  fs.readFileSync(
    path.join(ROOT, "build-spec/testing/fixtures/g2-non-subgroup.json"),
    "utf8"
  )
);

describe("Phase 1.1 BPF Probes", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idl = JSON.parse(
    fs.readFileSync(
      path.join(ROOT, "target/idl/alea_verifier.json"),
      "utf8"
    )
  );
  const program = new Program(idl, provider);

  const CU_LIMIT = ComputeBudgetProgram.setComputeUnitLimit({
    units: 1_400_000,
  });

  describe("Task 1.1.D: Fq::pow CU Benchmark", () => {
    it("measures CU for pow, sqrt, and inverse", async () => {
      const tx = await program.methods
        .probeCu()
        .accounts({ signer: provider.wallet.publicKey })
        .preInstructions([CU_LIMIT])
        .rpc({ commitment: "confirmed" });

      const txDetails = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });

      console.log("\n=== CU Benchmark Results ===");
      const logs = txDetails?.meta?.logMessages || [];
      for (const log of logs) {
        console.log(log);
      }

      expect(txDetails).to.not.be.null;
      expect(txDetails!.meta!.err).to.be.null;
    });
  });

  describe("CU Optimization: sqrt-and-check vs naive", () => {
    it("compares naive Legendre+sqrt vs optimized single-pow approach", async () => {
      const tx = await program.methods
        .probeOptimized()
        .accounts({ signer: provider.wallet.publicKey })
        .preInstructions([CU_LIMIT])
        .rpc({ commitment: "confirmed" });

      const txDetails = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });

      console.log("\n=== Optimization Comparison ===");
      const logs = txDetails?.meta?.logMessages || [];
      for (const log of logs) {
        console.log(log);
      }

      expect(txDetails).to.not.be.null;
      expect(txDetails!.meta!.err).to.be.null;
    });
  });

  describe("Task 1.1.B: G2 Subgroup Check", () => {
    it("accepts real evmnet pubkey (subgroup=true)", async () => {
      const pubkeyHex: string = chainInfo.public_key;
      const pubkeyBytes = Buffer.from(pubkeyHex, "hex");
      expect(pubkeyBytes.length).to.equal(128);

      const tx = await program.methods
        .probeG2(pubkeyBytes)
        .accounts({ signer: provider.wallet.publicKey })
        .preInstructions([CU_LIMIT])
        .rpc({ commitment: "confirmed" });

      const txDetails = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });

      const logs = txDetails?.meta?.logMessages || [];
      console.log("\n=== G2 Subgroup Check (real pubkey) ===");
      for (const log of logs) {
        console.log(log);
      }

      const resultLog = logs.find((l) => l.includes("in_subgroup"));
      expect(resultLog).to.include("in_subgroup=true");
    });

    it("rejects non-subgroup point (subgroup=false)", async () => {
      const nonSubgroupHex: string =
        g2NonSubgroup.point_g2_non_subgroup_hex;
      const nonSubgroupBytes = Buffer.from(nonSubgroupHex, "hex");
      expect(nonSubgroupBytes.length).to.equal(128);

      const tx = await program.methods
        .probeG2(nonSubgroupBytes)
        .accounts({ signer: provider.wallet.publicKey })
        .preInstructions([CU_LIMIT])
        .rpc({ commitment: "confirmed" });

      const txDetails = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });

      const logs = txDetails?.meta?.logMessages || [];
      console.log("\n=== G2 Subgroup Check (non-subgroup) ===");
      for (const log of logs) {
        console.log(log);
      }

      const resultLog = logs.find((l) => l.includes("in_subgroup"));
      expect(resultLog).to.include("in_subgroup=false");
    });
  });
});
