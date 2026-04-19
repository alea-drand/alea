# Persona: Dmitri Volkov — Senior Anchor Author

**Background:** 5 years Solana / Anchor. Author of 3 production programs (~$40M TVL: perps DEX, NFT marketplace, governance). Evaluating `alea-sdk` as a replacement for my current VRF integration. I pattern-match against `anchor-spl` ergonomics. I have been burned by Accounts derives that silently accept unowned accounts, CPI wrappers that discard error shapes, and examples that look right but teach antipatterns.

**Scope read:** `sdk/rust/` (full), `programs/alea-verifier/src/` (full), `programs/example-lottery/src/` (full), `README.md`, `CHANGELOG.md`, `LICENSE`.

**Success criteria:** Can I copy the example-lottery pattern directly into my perps DEX? Are the mandatory constraints un-missable? Does the one-liner CPI actually replace 10 lines of setup? No doc gaps on anything I would instantiate.

---

## Execution Notes

I read every file in the permitted scope before writing a single finding. I checked:

- `sdk/rust/src/{lib.rs, accounts.rs, cpi.rs, errors.rs}` — full SDK surface
- `programs/alea-verifier/src/{lib.rs, state.rs, errors.rs, events.rs}` — on-chain types
- `programs/alea-verifier/src/instructions/{verify.rs, initialize.rs, update_config.rs, map_to_point_debug.rs, mod.rs}` — instruction logic
- `programs/alea-verifier/src/crypto/pairing.rs` — cryptographic primitives
- `programs/example-lottery/src/{lib.rs, errors.rs}` — reference consumer
- `README.md`, `CHANGELOG.md`, `sdk/rust/README.md`, `sdk/rust/CAVEATS.md`

---

## Findings

### [T1] `sdk/rust/src/cpi.rs:58` — CPI helper accepts any AccountInfo for `config`; `seeds::program` enforcement is consumer-only and silently bypassable

The `alea_sdk::cpi::verify` function takes raw `AccountInfo` arguments. It constructs `CpiContext::new` directly without any re-validation that `config` is the legitimate Alea PDA. The mandatory `seeds::program = alea_program.key()` guard lives exclusively in the consumer's `#[derive(Accounts)]` struct. If a consumer calls `alea_sdk::cpi::verify` from a non-Anchor path (e.g., a raw `process_instruction` handler, a governance relay, or a CPI forwarder that has already deserialized accounts), the Anchor constraint never fires and an attacker can pass a fake Config PDA with attacker-controlled `pubkey_g2`. The SDK's own doc comments describe this correctly, but the function signature itself provides no mechanical barrier — passing the wrong account compiles and links without error. The ADR 0034 threat is real; the mitigation is entirely documentation-level on the CPI call side.

**Suggested fix:** At minimum, add a runtime check inside `cpi::verify` that asserts `config.owner == &crate::PROGRAM_ID`. This catches non-Anchor callers. The canonical Anchor path still gets the stronger `seeds::program` PDA re-derivation via the derive macro. A single `require!(config.owner == &crate::PROGRAM_ID, ...)` at the top of `verify` adds ~200 CU and closes the non-Anchor caller gap.

---

### [T1] `programs/example-lottery/src/lib.rs:150-167` — Direct lamport manipulation on a `mut`-borrowed PDA violates Anchor's post-instruction ownership check and may corrupt account state on program upgrades

The `resolve_bet` handler modifies lamports via `try_borrow_mut_lamports()` on the `bet` PDA rather than routing through `system_program::transfer`. This is a known footgun: Anchor's `close = player` constraint on the `bet` account (line 225) zeroes out the discriminator and transfers rent-exempt lamports at the end of the instruction handler via its own cleanup path. If the direct lamport subtraction (`bet -= amount`) races the `close` cleanup, the resulting lamport math produces an underflow that either panics in debug mode or silently wraps in release mode on older SBF toolchains. More concretely: `close = player` expects to move `bet.lamports()` to `player` at instruction end. If the handler already drained `amount` from `bet` via direct borrow, `close` still tries to move the original `rent_exempt + amount` lamports and the subtraction underflows if `amount > remaining_lamports`. This is a real funds-at-risk path for any bet size above dust. The example-lottery is labeled test-only but the doc comment in `lib.rs:11` says "canonical Alea CPI consumer" — consumers will copy this pattern.

**Suggested fix:** Use `system_program::transfer` with a PDA signer for payout. If using direct borrow manipulation is intentional (Anchor's `close` constraint is compatible with it in specific orderings), add an explicit comment explaining the invariant and a `debug_assert` that `bet.lamports() >= amount` before the subtraction. Alternatively, remove `close = player` and handle closure manually.

---

### [T2] `sdk/rust/src/lib.rs:162` — `clock.unix_timestamp as u64` silent negative-to-large cast

`is_round_recent` casts `clock.unix_timestamp` (an `i64`) to `u64` without bounds checking. On a live validator `unix_timestamp` is always positive, but Solana's Clock sysvar spec does not contractually guarantee this. A negative timestamp (e.g., from a misconfigured validator, a localnet clock quirk, or a future protocol edge) would cast to a very large `u64`, making `current_timestamp.saturating_sub(round_timestamp)` produce a value near `u64::MAX`, which is always greater than any `max_age_seconds`. The function would then return `false` (stale) for every valid round, causing all verify calls to fail until the clock normalizes. This is availability-impacting rather than exploitable, but in a high-frequency trading context it would halt all randomness consumption. The fix is `clock.unix_timestamp.max(0) as u64` or an explicit `require!(clock.unix_timestamp >= 0, ...)` in the calling instruction.

---

### [T2] `programs/example-lottery/src/lib.rs:54-65` — `min_allowed_round` derivation uses integer division that can produce off-by-one; attacker can submit round = current_round

The formula `min_allowed_round = (current_ts - genesis_time) / period + 1` uses `saturating_div` which truncates. The intent is that the round must be emitted _after_ commit. However, if `current_ts` falls exactly on a round boundary (i.e., `(current_ts - genesis_time) % period == 0`), then `(current_ts - genesis_time) / period` is the current in-progress round, and `+ 1` gives the next round — which is correct. But if `current_ts` is mid-period (e.g., 1 second after a round was emitted), the division truncates to the already-emitted round number, and `+ 1` gives the next future round — also correct. The +1 holds in all cases due to integer truncation, so this is actually not exploitable under normal conditions. However, the code uses `min_allowed_ts = current_ts + period` before dividing, which adds an extra period of margin beyond what the comment says. The comment says "one period AFTER commit" but the actual minimum guaranteed gap is up to 2 periods. Consumers copying this pattern for time-sensitive applications (e.g., 3-second drand rounds) will experience unnecessary resolution delays. This is a DX issue in the reference example.

---

### [T2] `sdk/rust/src/cpi.rs` — No `system_program` account in the CPI accounts struct; relies on Alea's `verify` instruction not requiring it, but this is invisible to consumers

The `alea_verifier::cpi::accounts::Verify` struct contains only `config` and `payer`. Consumers who glance at the CPI helper signature see three `AccountInfo` args (program, config, payer) and naturally assume that is the complete required account set. If Alea adds a new required account to `Verify` in a future version (e.g., a fee collector or a rate-limit PDA), the CPI helper's signature will change. Because `alea-sdk` uses an exact-pinned `version = "=0.1.0"` dependency on `alea-verifier`, this won't silently compile wrong — any version bump forces both to update together. This is actually well-handled by the exact pin. Flagging as T2 because the exact-pin protocol is only documented in `Cargo.toml` comments; if a consumer forks the SDK or vendor-patches it, they could break this invariant silently. The doc comment should explicitly state the exact-pin rationale.

---

### [T2] `programs/alea-verifier/src/errors.rs:38-43` — `InvalidFieldElement` (6003) is documented as "reserved / currently unreachable" but the public doc comment says "Field element is not in the valid range" — consumer error handling will misclassify if this ever fires

The `#[msg]` text for 6003 says "Field element is not in the valid range" — implying an input validation error that a retry might fix. The inline comment says it is currently unreachable and retained for future use. If a consumer builds a retry loop that treats 6003 as a transient error (retryable), and 6003 fires for an unexpected reason, the retry loop will spin forever. The `#[msg]` text should match the actual semantics: "Reserved — currently unreachable; treat as PairingError (non-retryable)" or similar.

---

### [T3] `programs/alea-verifier/src/state.rs:3` — Doc comment says "evmnet" (drand chain name) twice, but the field comment on `pubkey_g2` says "Kyber byte ordering" which is not standard Anchor/BN254 terminology

Consumers integrating G2 points from other sources (e.g., a TS script that calls the drand API directly) will not know what "Kyber byte ordering" means without chasing external docs. The actual encoding is `x_c1 || x_c0 || y_c1 || y_c0` per EIP-197, which is more recognizable. Rename the comment to reference EIP-197 for recognizability in the Anchor/Solana ecosystem.

---

### [T3] `sdk/rust/README.md:36` — Quick-start example imports `use alea_sdk::{self, AleaVerify}` but `AleaVerify` is not a public export; the correct type is `AleaVerifier`

Line 36 of `sdk/rust/README.md` imports `AleaVerify` (without the `r`). The actual public export from `sdk/rust/src/lib.rs:103` is `AleaVerifier`. This will produce a compile error for anyone copy-pasting the README quick start. The `lib.rs` doc example and the `programs/example-lottery` reference both use the correct `AleaVerifier`, so this is an isolated typo in the standalone SDK README. Low blast radius but will be the first thing any new integrator hits.

---

### [T3] `sdk/rust/src/lib.rs:44` — Quick-start example uses `.try_into().unwrap()` without explanation; consumers unfamiliar with Rust slice-to-array conversion will cargo-cult unwrap into production code

The line `randomness[0..8].try_into().unwrap()` derives a `[u8; 8]` from a slice. The `unwrap()` cannot fail here because `randomness` is always `[u8; 32]` and a 0..8 slice always converts to `[u8; 8]`, but the pattern trains consumers to `unwrap()` on try_into. A brief comment "infallible: randomness is always [u8; 32]" or using `<[u8; 8]>::try_from(...).expect(...)` with an explanation would reduce unwrap-copy-paste cargo-culting in consumer programs.

---

## Summary

The SDK is in good shape for an early-stage public good. The `seeds::program` mandatory constraint is clearly documented in three places (lib.rs, README, example). The CPI one-liner correctly wraps the return-data capture footgun. The error table is clean and the exact-pin versioning strategy is sound.

Two findings warrant attention before promoting to production consumers. The most actionable is the raw `AccountInfo` owner check gap in `cpi::verify` (T1-1): non-Anchor callers get no mechanical enforcement of the PDA ownership invariant, relying entirely on docs. The lamport manipulation pattern in `example-lottery` (T1-2) is the canonical template consumers will copy; the `close = player` + direct borrow interaction is a latent funds-at-risk path that should either be fixed or defended with explicit invariant comments before the example is cited as production-safe.
