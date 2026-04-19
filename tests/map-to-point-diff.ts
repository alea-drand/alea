// T1.04 — BPF-vs-gnark `map_to_point` parity test.
//
// Closes Phase 2.5 T1.04: "Native-vs-BPF map_to_point parity never
// differentially tested." Transitive proof:
//   1. NATIVE map_to_point == gnark-crypto MapToG1
//      (proven by src/crypto/svdw.rs:260+301 unit tests — 8 byte-equality
//      asserts against the same gnark fixtures this test uses)
//   2. [THIS TEST] BPF map_to_point == gnark-crypto MapToG1
//      (direct byte-equality: calls map_to_point_debug on BPF and
//      compares against fixture Q0/Q1 values)
// => BPF == NATIVE transitively.
//
// Fixture u0/u1 + expected Q0/Q1 values were generated from drand rounds
// 1 and 9337227 using gnark-crypto's MapToG1 reference implementation,
// then inlined here so this test file is self-contained.

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ComputeBudgetProgram } from "@solana/web3.js";
import { expect } from "chai";

import aleaIdl from "../target/idl/alea_verifier.json";

// evmnet constants (subset — only what we need for the debug instruction,
// which needs no Config account — just a fee payer).

interface ParityCase {
  round: number;
  u_hex: string;
  expected_qx_hex: string;
  expected_qy_hex: string;
  description: string;
}

// 4 cases: u0 and u1 from each of drand round 1 and round 9337227.
// Expected Q0/Q1 values were generated from gnark-crypto/ecc/bn254
// MapToG1 as an independent cross-check reference.
const PARITY_CASES: ParityCase[] = [
  {
    round: 1,
    u_hex: "1b163e041c11b8ddb908e7b705c98ca4f393243bf3664bf5934a3680d3a5bfc6",
    expected_qx_hex:
      "1e10b19957a0ab51d8ed02605e5fdb691f78e287817525ed109cb0b5b2519723",
    expected_qy_hex:
      "0742fdfa5dba51b9c799434e73fbb705930d9e29cefad99b31f7255b0d62d370",
    description: "round 1 u0 -> Q0",
  },
  {
    round: 1,
    u_hex: "0b2f337436437aef114e4f8383ac665c24fe4d3f88b3c53d494ad4104b9d15eb",
    expected_qx_hex:
      "15b1de83d800a488b346a8e46b60404911b9e24f8f0ce295fb1940f2e81fe902",
    expected_qy_hex:
      "21e341fa458ee12634b567e980ff1561fba99ef9e6858e30373b2bb5b3fb2ccf",
    description: "round 1 u1 -> Q1",
  },
  {
    round: 9337227,
    u_hex: "109ead626603ce780c14be70861676828e42948357c960d53e4250cb47246064",
    expected_qx_hex:
      "0bdac09968c4675115f5173ed5a2af9da4dd42dea8d82824cd45d4e40c52f4c3",
    expected_qy_hex:
      "1db41b01f6e7a7e1463e4eb6dd35ffd39deca11bf020262592c2f2e3a9e871e2",
    description: "round 9337227 u0 -> Q0",
  },
  {
    round: 9337227,
    u_hex: "1da61ba0e660ae1d421c04d6aa2a5d69b24a1a1d380d01b464bdf315b080e781",
    expected_qx_hex:
      "2c547cc28601f4c5376d75d935d493dcde85f549ed79c1d136227fa7588a09d8",
    expected_qy_hex:
      "1116342a64c29038836c8b7b8c1270ca8af9535ca542a0aee6d6b82855157ad3",
    description: "round 9337227 u1 -> Q1",
  },
];

// CU budget for map_to_point only: ~15-30K observed. 200K is the default
// per-instruction budget and is more than enough; 1M gives breathing room
// if any optimization shifts CU.
const CU_LIMIT_IX = ComputeBudgetProgram.setComputeUnitLimit({
  units: 1_000_000,
});

describe("T1.04 — BPF map_to_point ↔ gnark-crypto parity", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const aleaProgram = new Program(aleaIdl as anchor.Idl, provider);

  for (const c of PARITY_CASES) {
    it(`T1.04 ${c.description} — BPF output equals gnark`, async () => {
      const u_bytes = Array.from(Buffer.from(c.u_hex, "hex"));
      expect(u_bytes.length, "u input must be 32 bytes").to.equal(32);

      const tx = await aleaProgram.methods
        .mapToPointDebug(u_bytes)
        .accounts({
          payer: provider.wallet.publicKey,
        })
        .preInstructions([CU_LIMIT_IX])
        .rpc({ commitment: "confirmed", skipPreflight: true });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      expect(info?.meta?.err, "map_to_point_debug must succeed").to.equal(null);

      const [retB64] = (info!.meta as any).returnData.data as [string, string];
      const output = Buffer.from(retB64, "base64");
      expect(
        output.length,
        "map_to_point_debug must return 64 bytes (x || y)",
      ).to.equal(64);

      const actual_qx_hex = output.slice(0, 32).toString("hex");
      const actual_qy_hex = output.slice(32, 64).toString("hex");

      expect(
        actual_qx_hex,
        `${c.description}: BPF Q.x must match gnark reference`,
      ).to.equal(c.expected_qx_hex);
      expect(
        actual_qy_hex,
        `${c.description}: BPF Q.y must match gnark reference`,
      ).to.equal(c.expected_qy_hex);
    });
  }
});
