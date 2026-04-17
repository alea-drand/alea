// Alea Phase 2 — Localnet integration tests (12 P0 + 3 P1).
//
// Covers program/spec.md acceptance criteria AC-1..AC-19:
//   initialize (happy/wrong chain_hash/wrong pubkey/duplicate)
//   verify (round 1 + round 9337227 fixtures, round 0, corrupt sig,
//     non-canonical G1, event emission)
//   update_config (happy, wrong authority -> 2001)
//   CPI (cpi-consumer calls alea.verify, receives 32-byte return data)
//   seeds::program constraint smoke (ADR 0034)
//
// Error codes reference program/spec.md §'Error Codes' (canonical),
// NOT phases/phase-2-program.md bullet numbers (stale).

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  ComputeBudgetProgram,
  SystemProgram,
} from "@solana/web3.js";
import { expect } from "chai";
import { createHash } from "crypto";

import aleaIdl from "../target/idl/alea_verifier.json";
import cpiConsumerIdl from "../target/idl/cpi_consumer.json";

// ---------------------------------------------------------------------------
// evmnet chain parameters — must match crypto/constants.rs
// ---------------------------------------------------------------------------

const EVMNET_PUBKEY = Buffer.from(
  "07e1d1d335df83fa98462005690372c643340060d205306a9aa8106b6bd0b382" +
    "0557ec32c2ad488e4d4f6008f89a346f18492092ccc0d594610de2732c8b808f" +
    "0095685ae3a85ba243747b1b2f426049010f6b73a0cf1d389351d5aaaa1047f6" +
    "297d3a4f9749b33eb2d904c9d9ebf17224150ddd7abd7567a9bec6c74480ee0b",
  "hex",
);

const EVMNET_CHAIN_HASH = Buffer.from(
  "04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3",
  "hex",
);

const EVMNET_GENESIS = new BN(1727521075);
const EVMNET_PERIOD = new BN(3);

// ---------------------------------------------------------------------------
// Drand fixtures — match crypto/pairing.rs tests
// ---------------------------------------------------------------------------

const ROUND_1_SIG = Buffer.from(
  "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346" +
    "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373",
  "hex",
);
const ROUND_1_RANDOMNESS =
  "781b75698adc3af62cfa55db83cf0c73ae54e1ac8c0d4c3a2224126b65369ec5";

const ROUND_9337227_SIG = Buffer.from(
  "01d65d6128f4b2df3d08de85543d8efe06b0281d0770246ae3672e8ddd3efda0" +
    "269373123458f0b5c0073eeed1c816a06809e127421513e34ee07df6987910b3",
  "hex",
);
const ROUND_9337227_RANDOMNESS =
  "a1e645cd6193837f626716851f5c42ad4bf63ad75193b2cae40f88c08c8f3bd8";

// SDK default per T2.A — enough headroom for SVDW + pairing + consumer logic.
// Tests use the 1.4M ceiling for measurement breathing room; the 900K SDK
// default is asserted separately in Wave 11 CU benchmark.
const CU_LIMIT_IX = ComputeBudgetProgram.setComputeUnitLimit({
  units: 1_400_000,
});

// Extract Anchor error code from a caught error. Works for both direct
// errors (require! guards) and CPI-surfaced errors.
function errCode(err: any): number | undefined {
  return (
    err?.error?.errorCode?.number ??
    err?.errorCode?.number ??
    err?.code
  );
}

describe("Alea Phase 2 — Localnet Integration", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const aleaProgram = new Program(aleaIdl as anchor.Idl, provider);
  const cpiConsumerProgram = new Program(
    cpiConsumerIdl as anchor.Idl,
    provider,
  );

  const [configPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    aleaProgram.programId,
  );

  // =========================================================================
  // initialize — tests 1-4 (happy / wrong chain_hash / wrong pubkey / dup)
  // =========================================================================
  describe("initialize", () => {
    // NOTE: ordering matters. Failure tests run first (PDA not created),
    // then happy path (PDA created), then duplicate (fails — Anchor built-in).

    it("P0#2 rejects wrong chain_hash with 6007 WrongChainHash", async () => {
      const wrongChainHash = Buffer.alloc(32, 0xff);
      try {
        await aleaProgram.methods
          .initialize(
            Array.from(EVMNET_PUBKEY),
            EVMNET_GENESIS,
            EVMNET_PERIOD,
            Array.from(wrongChainHash),
          )
          .accounts({
            config: configPda,
            authority: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("Should have rejected wrong chain_hash");
      } catch (err: any) {
        expect(errCode(err)).to.equal(
          6007,
          `Expected 6007 WrongChainHash, got: ${err?.message ?? err}`,
        );
      }
    });

    it("P0#3 rejects wrong pubkey_g2 with 6008 WrongPubkey (fallback path)", async () => {
      const wrongPubkey = Buffer.alloc(128, 0xab);
      try {
        await aleaProgram.methods
          .initialize(
            Array.from(wrongPubkey),
            EVMNET_GENESIS,
            EVMNET_PERIOD,
            Array.from(EVMNET_CHAIN_HASH),
          )
          .accounts({
            config: configPda,
            authority: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("Should have rejected wrong pubkey_g2");
      } catch (err: any) {
        expect(errCode(err)).to.equal(
          6008,
          `Expected 6008 WrongPubkey, got: ${err?.message ?? err}`,
        );
      }
    });

    it("P0#1 happy path — config PDA created with all 6 fields", async () => {
      await aleaProgram.methods
        .initialize(
          Array.from(EVMNET_PUBKEY),
          EVMNET_GENESIS,
          EVMNET_PERIOD,
          Array.from(EVMNET_CHAIN_HASH),
        )
        .accounts({
          config: configPda,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const config: any = await (aleaProgram.account as any).config.fetch(
        configPda,
      );
      expect(Buffer.from(config.pubkeyG2).equals(EVMNET_PUBKEY)).to.equal(
        true,
        "pubkey_g2 must be stored exactly",
      );
      expect(config.genesisTime.toString()).to.equal(
        EVMNET_GENESIS.toString(),
      );
      expect(config.period.toString()).to.equal(EVMNET_PERIOD.toString());
      expect(Buffer.from(config.chainHash).equals(EVMNET_CHAIN_HASH)).to.equal(
        true,
      );
      expect(config.authority.toBase58()).to.equal(
        provider.wallet.publicKey.toBase58(),
      );
      expect(config.bump).to.be.a("number");
    });

    it("P0#4 duplicate initialize rejected by Anchor 'account already in use'", async () => {
      try {
        await aleaProgram.methods
          .initialize(
            Array.from(EVMNET_PUBKEY),
            EVMNET_GENESIS,
            EVMNET_PERIOD,
            Array.from(EVMNET_CHAIN_HASH),
          )
          .accounts({
            config: configPda,
            authority: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("Duplicate initialize must fail");
      } catch (err: any) {
        const msg = String(err?.message ?? err).toLowerCase();
        expect(
          msg.includes("already in use") || msg.includes("0x0"),
          `Expected Anchor 'account already in use', got: ${err?.message ?? err}`,
        ).to.equal(true);
      }
    });
  });

  // =========================================================================
  // verify — tests 5-9 + 14 (fixtures + error paths + event emission)
  // =========================================================================
  describe("verify", () => {
    it("P0#5 round-1 fixture produces byte-matching drand randomness", async () => {
      const tx = await aleaProgram.methods
        .verify(new BN(1), Array.from(ROUND_1_SIG))
        .accounts({
          config: configPda,
          payer: provider.wallet.publicKey,
        })
        .preInstructions([CU_LIMIT_IX])
        .rpc({ commitment: "confirmed", skipPreflight: true });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      expect(info?.meta?.err).to.equal(null, "tx must succeed");

      const returnData = (info?.meta as any)?.returnData;
      expect(returnData, "verify must set program return data").to.not.be
        .undefined;
      const [retB64] = returnData.data as [string, string];
      const bytes = Buffer.from(retB64, "base64");
      expect(bytes.length).to.equal(32, "randomness must be 32 bytes");
      expect(bytes.toString("hex")).to.equal(
        ROUND_1_RANDOMNESS,
        "round 1 randomness must match drand API byte-for-byte",
      );
    });

    it("P0#6 round-9337227 fixture produces byte-matching drand randomness", async () => {
      const tx = await aleaProgram.methods
        .verify(new BN(9337227), Array.from(ROUND_9337227_SIG))
        .accounts({
          config: configPda,
          payer: provider.wallet.publicKey,
        })
        .preInstructions([CU_LIMIT_IX])
        .rpc({ commitment: "confirmed", skipPreflight: true });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      const [retB64] = (info!.meta as any).returnData.data as [
        string,
        string,
      ];
      expect(Buffer.from(retB64, "base64").toString("hex")).to.equal(
        ROUND_9337227_RANDOMNESS,
      );
    });

    it("P0#8 round=0 rejected with 6002 RoundZero", async () => {
      try {
        await aleaProgram.methods
          .verify(new BN(0), Array.from(ROUND_1_SIG))
          .accounts({
            config: configPda,
            payer: provider.wallet.publicKey,
          })
          .preInstructions([CU_LIMIT_IX])
          .rpc();
        expect.fail("round 0 must be rejected");
      } catch (err: any) {
        expect(errCode(err)).to.equal(6002);
      }
    });

    it("P0#9 non-canonical G1 (x = p) rejected with 6001 InvalidG1Point", async () => {
      const pBytes = Buffer.from(
        "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47",
        "hex",
      );
      const nonCanonicalSig = Buffer.concat([pBytes, Buffer.alloc(32, 0)]);
      try {
        await aleaProgram.methods
          .verify(new BN(1), Array.from(nonCanonicalSig))
          .accounts({
            config: configPda,
            payer: provider.wallet.publicKey,
          })
          .preInstructions([CU_LIMIT_IX])
          .rpc();
        expect.fail("non-canonical G1 must be rejected");
      } catch (err: any) {
        expect(errCode(err)).to.equal(6001);
      }
    });

    it("P0#7 corrupt signature rejected with 6000 or 6001", async () => {
      const corrupted = Buffer.from(ROUND_1_SIG);
      corrupted[0] ^= 0xff;
      try {
        await aleaProgram.methods
          .verify(new BN(1), Array.from(corrupted))
          .accounts({
            config: configPda,
            payer: provider.wallet.publicKey,
          })
          .preInstructions([CU_LIMIT_IX])
          .rpc();
        expect.fail("corrupt sig must be rejected");
      } catch (err: any) {
        const code = errCode(err);
        expect([6000, 6001]).to.include(
          code,
          `Expected 6000 or 6001, got ${code}`,
        );
      }
    });

    it("P1#14 BeaconVerified event emitted with payer = tx signer", async () => {
      const tx = await aleaProgram.methods
        .verify(new BN(1), Array.from(ROUND_1_SIG))
        .accounts({
          config: configPda,
          payer: provider.wallet.publicKey,
        })
        .preInstructions([CU_LIMIT_IX])
        .rpc({ commitment: "confirmed", skipPreflight: true });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      const logs = info?.meta?.logMessages ?? [];
      const programDataLog = logs.find((l) => l.startsWith("Program data:"));
      expect(
        programDataLog,
        "BeaconVerified must emit a 'Program data:' log",
      ).to.not.be.undefined;

      // Decode the event via Anchor's event coder
      const coder = new anchor.BorshEventCoder(aleaIdl as anchor.Idl);
      const payload = programDataLog!.replace("Program data: ", "");
      const event = coder.decode(payload);
      expect(event?.name).to.equal("BeaconVerified");
      expect((event!.data as any).round.toString()).to.equal("1");
      expect((event!.data as any).payer.toBase58()).to.equal(
        provider.wallet.publicKey.toBase58(),
      );
      expect(
        Buffer.from((event!.data as any).randomness).toString("hex"),
      ).to.equal(ROUND_1_RANDOMNESS);
    });
  });

  // =========================================================================
  // update_config — tests 10, 11
  // =========================================================================
  describe("update_config", () => {
    it("P0#11 wrong authority rejected with Anchor 2001 ConstraintHasOne", async () => {
      const impostor = Keypair.generate();
      // Fund the impostor so it can pay fees
      const sig = await provider.connection.requestAirdrop(
        impostor.publicKey,
        1_000_000_000,
      );
      await provider.connection.confirmTransaction(sig, "confirmed");

      try {
        await aleaProgram.methods
          .updateConfig(
            Array.from(EVMNET_PUBKEY),
            EVMNET_GENESIS,
            EVMNET_PERIOD,
            Array.from(EVMNET_CHAIN_HASH),
          )
          .accounts({
            config: configPda,
            authority: impostor.publicKey,
          })
          .signers([impostor])
          .rpc();
        expect.fail("Wrong authority must be rejected");
      } catch (err: any) {
        // Anchor ConstraintHasOne = 2001
        expect(errCode(err)).to.equal(2001);
      }
    });

    it("P0#10 happy path — update_config no-op returns Ok + NO event (T2.D idempotency)", async () => {
      // T2.D (Wave E, commit 8166f12) — update_config_handler early-returns
      // without emitting ConfigUpdated if all four fields match stored values.
      // Eliminates the event-spam attack surface where a compromised authority
      // could flood indexers with no-op updates (P06-T2-01).
      //
      // T2.E (Wave E, commit 8166f12) — all four Config fields must equal
      // EXPECTED_EVMNET_* constants (new error codes 6010/6011 for
      // genesis_time/period). So the ONLY successful update_config call
      // passes byte-identical values, which triggers T2.D idempotency and
      // returns Ok without emitting an event. In practice update_config
      // is operationally a no-op under ADR 0027 single-chain design; the
      // instruction + event schema exist for ADR 0028 CPI stability.
      //
      // Pre-T2.D: no-op update fired ConfigUpdated.
      // Post-T2.D: no-op update succeeds silently (tx ok, no event).
      const tx = await aleaProgram.methods
        .updateConfig(
          Array.from(EVMNET_PUBKEY),
          EVMNET_GENESIS,
          EVMNET_PERIOD,
          Array.from(EVMNET_CHAIN_HASH),
        )
        .accounts({
          config: configPda,
          authority: provider.wallet.publicKey,
        })
        .rpc({ commitment: "confirmed" });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      const logs = info?.meta?.logMessages ?? [];
      const programDataLog = logs.find((l) => l.startsWith("Program data:"));
      expect(programDataLog, "T2.D: no ConfigUpdated event on no-op update").to
        .be.undefined;
    });

    // T2.F — update_config error-code coverage for byte-equality guards
    // (Wave E commit 8166f12 — T2.E introduced byte-equality checks on all
    // four Config fields. These tests exercise each guard path.)
    //
    // Guard order in update_config_handler (programs/alea-verifier/src/instructions/update_config.rs):
    //   1. chain_hash    != EXPECTED_EVMNET_CHAIN_HASH   -> 6007 WrongChainHash
    //   2. pubkey_g2     != EXPECTED_EVMNET_PUBKEY       -> 6008 WrongPubkey
    //   3. genesis_time  != EXPECTED_EVMNET_GENESIS_TIME -> 6010 InvalidGenesisTime
    //   4. period        != EXPECTED_EVMNET_PERIOD       -> 6011 InvalidPeriod
    // Each test passes 3 correct values + 1 wrong so the single-field guard fires.
    it("T2.F#1 wrong chain_hash rejected with 6007 WrongChainHash", async () => {
      const wrongChainHash = Buffer.alloc(32, 0xff);
      try {
        await aleaProgram.methods
          .updateConfig(
            Array.from(EVMNET_PUBKEY),
            EVMNET_GENESIS,
            EVMNET_PERIOD,
            Array.from(wrongChainHash),
          )
          .accounts({
            config: configPda,
            authority: provider.wallet.publicKey,
          })
          .rpc();
        expect.fail("wrong chain_hash must be rejected");
      } catch (err: any) {
        expect(errCode(err)).to.equal(6007);
      }
    });

    it("T2.F#2 wrong pubkey_g2 rejected with 6008 WrongPubkey", async () => {
      const wrongPubkey = Buffer.alloc(128, 0xff);
      try {
        await aleaProgram.methods
          .updateConfig(
            Array.from(wrongPubkey),
            EVMNET_GENESIS,
            EVMNET_PERIOD,
            Array.from(EVMNET_CHAIN_HASH),
          )
          .accounts({
            config: configPda,
            authority: provider.wallet.publicKey,
          })
          .rpc();
        expect.fail("wrong pubkey_g2 must be rejected");
      } catch (err: any) {
        expect(errCode(err)).to.equal(6008);
      }
    });

    it("T2.F#3 wrong genesis_time rejected with 6010 InvalidGenesisTime", async () => {
      // Classic "period=0 consumer DoS under compromised auth" scenario:
      // passing 0 for genesis_time would bypass any consumer freshness check
      // that relies on (now - genesis_time) / period. T2.E closes this.
      const wrongGenesis = new BN(0);
      try {
        await aleaProgram.methods
          .updateConfig(
            Array.from(EVMNET_PUBKEY),
            wrongGenesis,
            EVMNET_PERIOD,
            Array.from(EVMNET_CHAIN_HASH),
          )
          .accounts({
            config: configPda,
            authority: provider.wallet.publicKey,
          })
          .rpc();
        expect.fail("wrong genesis_time must be rejected");
      } catch (err: any) {
        expect(errCode(err)).to.equal(6010);
      }
    });

    it("T2.F#4 wrong period rejected with 6011 InvalidPeriod", async () => {
      // Period=0 would cause divide-by-zero in any consumer freshness
      // check that computes (now - genesis_time) / period; period=9999
      // here is just a non-3 value to exercise the byte-inequality path.
      const wrongPeriod = new BN(9999);
      try {
        await aleaProgram.methods
          .updateConfig(
            Array.from(EVMNET_PUBKEY),
            EVMNET_GENESIS,
            wrongPeriod,
            Array.from(EVMNET_CHAIN_HASH),
          )
          .accounts({
            config: configPda,
            authority: provider.wallet.publicKey,
          })
          .rpc();
        expect.fail("wrong period must be rejected");
      } catch (err: any) {
        expect(errCode(err)).to.equal(6011);
      }
    });
  });

  // =========================================================================
  // CPI — tests 12, 15 (cpi-consumer + seeds::program smoke)
  // =========================================================================
  describe("CPI (cpi-consumer)", () => {
    it("P0#12 cpi-consumer calls alea.verify and receives 32-byte return data", async () => {
      const tx = await cpiConsumerProgram.methods
        .consumeRandomness(new BN(1), Array.from(ROUND_1_SIG))
        .accounts({
          aleaProgram: aleaProgram.programId,
          aleaConfig: configPda,
          payer: provider.wallet.publicKey,
        })
        .preInstructions([CU_LIMIT_IX])
        .rpc({ commitment: "confirmed", skipPreflight: true });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      expect(info?.meta?.err).to.equal(null);

      // The OUTER return data = cpi-consumer's result (which is the same
      // 32-byte randomness it received from alea). This proves Pattern A
      // (ADR 0030): Anchor 0.30.x auto-serialize of Result<[u8; 32]>.
      const [retB64] = (info!.meta as any).returnData.data as [
        string,
        string,
      ];
      expect(Buffer.from(retB64, "base64").toString("hex")).to.equal(
        ROUND_1_RANDOMNESS,
      );
    });

    it("T2.R CPI randomness = sha256(signature) — local byte-equality check", async () => {
      // T2.R (Wave I) — extends P0#12 by computing sha256(sig) in the test
      // and asserting byte-equality with the CPI return data. Makes the
      // "randomness = sha256(sig)" contract (ADR 0036) testable locally
      // without round-tripping through drand's published randomness.
      const tx = await cpiConsumerProgram.methods
        .consumeRandomness(new BN(1), Array.from(ROUND_1_SIG))
        .accounts({
          aleaProgram: aleaProgram.programId,
          aleaConfig: configPda,
          payer: provider.wallet.publicKey,
        })
        .preInstructions([CU_LIMIT_IX])
        .rpc({ commitment: "confirmed", skipPreflight: true });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      const [retB64] = (info!.meta as any).returnData.data as [string, string];
      const returnedRandomness = Buffer.from(retB64, "base64");

      // Locally compute sha256(signature) — the contract says this equals
      // the returned randomness (ADR 0036: NOT keccak256).
      const expectedRandomness = createHash("sha256")
        .update(ROUND_1_SIG)
        .digest();

      expect(
        returnedRandomness.toString("hex"),
        "CPI return data MUST equal sha256(signature) per ADR 0036",
      ).to.equal(expectedRandomness.toString("hex"));

      // Belt-and-suspenders: also equal to drand's published randomness
      // (end-to-end check remains green).
      expect(returnedRandomness.toString("hex")).to.equal(ROUND_1_RANDOMNESS);
    });

    it("T2.V BeaconVerified event inside CPI carries outer signer as payer", async () => {
      // T2.V (Wave I) — Alea emits BeaconVerified inside the CPI call. The
      // event's `payer` field reflects ctx.accounts.payer.key() (the Signer
      // account passed by cpi-consumer into the CPI accounts struct). In
      // the P0#12 flow the outer signer (provider.wallet) IS that payer, so
      // we assert the event surfaces it cleanly through the CPI boundary.
      //
      // If cpi-consumer were to swap in a PDA-derived signer for privacy
      // (documented pattern in sdk/rust-cpi.md), event.payer would be the
      // PDA instead — consumers need to know this to build indexers.
      const tx = await cpiConsumerProgram.methods
        .consumeRandomness(new BN(9337227), Array.from(ROUND_9337227_SIG))
        .accounts({
          aleaProgram: aleaProgram.programId,
          aleaConfig: configPda,
          payer: provider.wallet.publicKey,
        })
        .preInstructions([CU_LIMIT_IX])
        .rpc({ commitment: "confirmed", skipPreflight: true });

      const info = await provider.connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      const logs = info?.meta?.logMessages ?? [];

      // BeaconVerified is emitted via `emit!` → Program data: <base64> log.
      // Multiple Program data lines can exist when CPI chains emit events
      // (cpi-consumer itself emits nothing today, but keep the filter
      // robust by picking the first one and asserting its decoded type).
      const programDataLog = logs.find((l) => l.startsWith("Program data:"));
      expect(
        programDataLog,
        "T2.V: BeaconVerified must appear in outer tx logs even when emitted from a CPI",
      ).to.not.be.undefined;

      const coder = new anchor.BorshEventCoder(aleaIdl as anchor.Idl);
      const payload = programDataLog!.replace("Program data: ", "");
      const event = coder.decode(payload);
      expect(event?.name).to.equal("BeaconVerified");
      expect((event!.data as any).round.toString()).to.equal("9337227");
      expect((event!.data as any).payer.toBase58()).to.equal(
        provider.wallet.publicKey.toBase58(),
      );
      expect(
        Buffer.from((event!.data as any).randomness).toString("hex"),
      ).to.equal(ROUND_9337227_RANDOMNESS);
    });

    it("P1#15 seeds::program constraint rejects wrong config PDA (ADR 0034 smoke)", async () => {
      // Derive a PDA from cpi-consumer's OWN program ID (not Alea's) —
      // should fail the seeds::program constraint.
      const [wrongConfigPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("config")],
        cpiConsumerProgram.programId,
      );
      try {
        await cpiConsumerProgram.methods
          .consumeRandomness(new BN(1), Array.from(ROUND_1_SIG))
          .accounts({
            aleaProgram: aleaProgram.programId,
            aleaConfig: wrongConfigPda,
            payer: provider.wallet.publicKey,
          })
          .preInstructions([CU_LIMIT_IX])
          .rpc();
        expect.fail("wrong PDA must be rejected by seeds::program");
      } catch (err: any) {
        // T2.B (Wave H, commit 4380d57) — cpi-consumer changed from
        // UncheckedAccount<'info> to Account<'info, AleaConfig>. Anchor now
        // performs account deserialization BEFORE the seeds::program
        // constraint fires. A PDA from the wrong program ID triggers
        // AccountOwnedByWrongProgram (3007) / AccountDiscriminatorMismatch
        // (3002) / AccountNotInitialized (3012) BEFORE the seeds::program
        // constraint's 2006 ConstraintSeeds code.
        //
        // Pre-T2.B: UncheckedAccount → seeds::program (2006) → 2xxx
        // Post-T2.B: Account<Config> → account deserialization error → 3xxx
        //
        // Both are correct rejections; T2.B is a belt-and-suspenders
        // strengthening (ADR 0034 seeds::program still mandatory AND
        // account type validation catches substitution at the deserialization
        // layer). Accept either 2xxx or 3xxx as valid rejection codes.
        const code = errCode(err);
        expect(
          code,
          `wrong PDA must trigger Anchor 2xxx seeds::program constraint OR 3xxx account deserialization error (got ${code})`,
        ).to.satisfy(
          (c: number) =>
            (c >= 2000 && c <= 2999) || (c >= 3000 && c <= 3999),
        );
      }
    });
  });
});
