# Priya Iyer — Solana Security Auditor Cold-Read Audit

**Persona:** Sr. Solana auditor, 7 yrs, Sec3/OtterSec/indie. Audit style: skeptical, trust-boundary first, no benefit of the doubt on consumer-layer assumptions.
**Date:** 2026-04-19
**Scope:** `sdk/rust/`, `programs/alea-verifier/src/`, `programs/example-lottery/src/`, top-level `README.md`, `CHANGELOG.md`, `LICENSE`.
**Out of scope (per brief):** `build-spec/`, `audit/`, `.github/`, vault notes, TS SDK.
**Artifact read:** 15 source files (~1,700 LoC crypto + ~400 LoC Anchor glue + 280 LoC consumer example).

---

## Execution Notes

Traced every public instruction's trust boundary. For each: who can sign, what is account-constrained by Anchor, what handler body requires, what syscalls/math are invoked, what return data + events surface. Cross-checked all six brief-mandated vectors. Read `verify_beacon_full` line-by-line. Diffed consumer SDK claims (`lib.rs` rustdoc + `CAVEATS.md` + `README.md`) against actual on-chain behavior. No code changes; cold-read only.

**Trust surfaces inspected:**
- `initialize` — UpgradeAuthority gate + evmnet-constant byte-equality (6007/6008/6010/6011/6012). Hardened. Correct seeds + init-only + fees-by-authority.
- `update_config` — `has_one = authority` (Anchor 2001) + same byte-equality guards + T2.D idempotency. Correct.
- `verify` — `seeds = [b"config"], bump = config.bump` (on-Alea side; re-derives against `crate::ID` implicitly, blocks fake-PDA-at-CPI). Signer-only payer. No state mutation.
- `map_to_point_debug` — stateless pure fn; documented zero attack surface; confirmed.
- SDK `is_round_recent` — saturating arithmetic throughout; `clock.unix_timestamp as u64` cast examined.
- `cpi::verify` wrapper — captures return data in same expression via `.get()` (Pattern A, ADR 0030). Safe.
- Consumer example `example-lottery` — correctly uses `seeds::program`, recency, immediate capture, commit-reveal with `min_resolution_round` floor.

---

## Findings

### [T1] — none

No exploitable-against-consumer vulnerabilities under the documented mandatory-constraints usage contract.

### [T2] `sdk/rust/src/lib.rs:158-164` — `is_round_recent` silently accepts FUTURE rounds

`is_round_recent` computes `current_timestamp.saturating_sub(round_timestamp) <= max_age_seconds`. When `round_timestamp > current_timestamp` (the claimed drand round hasn't happened yet per on-chain clock), the saturating-sub returns `0`, which is always `<= max_age_seconds`. The function returns `true` for any arbitrarily far-future round. Consumers reading the doc comment ("round's emission timestamp is within `max_age_seconds` of the current slot") reasonably expect a two-sided window; they get a one-sided past-only window. Attack vector: an attacker who obtains a drand signature that hasn't been broadcast on the drand API yet (or a leaked pre-release sig from a colluding drand member) can submit `round = very_large_future_round`, the recency check passes, pairing passes, consumer resolves with attacker-influenced randomness. Blast radius: narrow (requires unpublished drand sig) but defeats the recency guardrail's stated purpose. PoC: `is_round_recent(u64::MAX, &cfg, &clock, 30) == true` for any reasonable clock/cfg. Fix: add `round_timestamp <= current_timestamp + tolerance` upper bound, or gate with `round <= estimated_current_round + 1`.

### [T2] `sdk/rust/src/lib.rs:162` — signed→unsigned cast on `clock.unix_timestamp`

`clock.unix_timestamp as u64` silently wraps a theoretically-negative i64 (pre-epoch, or a future Firedancer bug returning negative) to a huge positive. Combined with `saturating_sub`, this could push recency arithmetic into nonsense territory (stale-pass for near-zero `round_timestamp`). In current mainnet realities Solana clock is ~1.7e9, so this is defensive-only. Hardening: `i64::try_into().unwrap_or(0)` or explicit `max(0, ts) as u64`. T2 not T1 because practical reachability is ~0 absent a validator bug.

### [T2] `programs/alea-verifier/src/events.rs:11-16` — `BeaconVerified.payer` doxxes end-user wallets in public logs

The `BeaconVerified` event records `payer: Pubkey` on every successful verify. For consumer programs that pass the end-user wallet directly as the CPI `payer` (the shape demonstrated in the README quick start + `example-lottery`'s `ResolveBet.payer`), every lottery participation / game resolution is trivially indexable — payer wallet + round number + resulting randomness form a public transcript. Privacy-sensitive consumers (sealed-bid auctions, anonymous lotteries) must route through a program-PDA signer — this is called out in `sdk/rust/README.md` §3 but not surfaced in the Rust lib.rs doc example OR enforced by any compile-time construct. The quick-start example code actively encourages the wrong pattern. Not directly exploitable but narrows the safety margin for privacy-by-default integrations. Recommend a `#[doc(warning)]` block above the lib.rs example or a dedicated `cpi::verify_with_pda_signer` helper.

### [T2] `programs/alea-verifier/src/instructions/verify.rs:20-28` — no `mut` on `payer`, no explicit fee-deduction write

`payer: Signer<'info>` is not marked `mut`. This is correct for the current verify semantics (Alea charges no fee, just requires a signature for tx attribution). But it means the `payer` account can be arbitrarily aliased — any signer on the tx works, not necessarily the user bearing the cost. Consumers that trust `BeaconVerified.payer` for analytics or fee accounting can be misled when multi-signer txs attribute the verify to whichever signer the consumer happened to route. T2 because it's a UX/analytics concern, not a balance-theft vector. Explicit: alt, a `signer2` could front the alea payer while a different `signer1` bears the user cost, and the event attributes to signer2.

### [T2] `programs/alea-verifier/src/instructions/verify.rs:39-83` — `verify_beacon_full` takes `&pubkey_g2` but never revalidates vs EXPECTED constant

Per ADR 0028 + the handler comment ("config.pubkey_g2 == EXPECTED_EVMNET_PUBKEY defense-in-depth considered and deliberately skipped"), the verify path trusts whatever pubkey_g2 is in the Config PDA. The invariant is enforced at `initialize` + `update_config` via byte-equality to `EXPECTED_EVMNET_PUBKEY`. This is sound **as long as no future update_config variant is added that relaxes the constraint**. Anyone extending the program with a new instruction that writes `config.pubkey_g2` (e.g., a future rotation path) would silently break verify without the compiler complaining. Defense-in-depth single `require!(config.pubkey_g2 == EXPECTED_EVMNET_PUBKEY)` at verify entry costs ~200 CU and closes the regression-by-future-contributor class. The 10%-margin comment in the code acknowledges this tradeoff; my audit recommendation is to reconsider — 200 CU is 0.04% of the 454K budget.

### [T3] `sdk/rust/src/lib.rs:162` — `max_age_seconds: u64` has no maximum-sanity clamp

A caller who passes `u64::MAX` (or a fat-fingered config value) disables the check entirely while the code-path looks correct. Tiny hardening: `debug_assert!(max_age_seconds < 3600)` or a doc line stating "values > ~60s effectively disable recency."

### [T3] `sdk/rust/src/lib.rs:158` — `is_round_recent` takes `&Config` by reference but the stored `period`/`genesis_time` are byte-pinned to EXPECTED_* constants — no need to read Config

Because initialize + update_config guards force `config.period == 3` and `config.genesis_time == 1_727_521_075`, reading these from the passed-in `&Config` is redundant — the SDK could use `alea_sdk::EXPECTED_PERIOD` / `alea_sdk::EXPECTED_GENESIS_TIME` constants and skip the Config account read entirely at the consumer layer. Save one account on the consumer's Accounts struct. Functional only (no security impact); T3 hardening.

### [T3] `programs/alea-verifier/src/errors.rs:28-97` — 4 documented-unreachable error codes (6003 `InvalidFieldElement`, 6005 `InvalidG2Point`, 6008 `WrongPubkey` — wait, 6008 IS reachable via initialize/update_config — 6009 `ReturnDataMissing`) reserved per ADR 0028

Reserved error codes are good CPI hygiene, but the README error-code table (sdk/rust/README.md:174-189) labels `6003` as `ChainHashMismatch`, `6005` as `InvalidChainHash`, `6007` as `InvalidPubkeyG2`, `6008` as `InvalidPublicKey`. This does NOT match `errors.rs` (where 6003=InvalidFieldElement, 6005=InvalidG2Point, 6007=WrongChainHash, 6008=WrongPubkey). Either the README or the code is wrong. Consumer integrators will match on the wrong codes. T3 because it's a doc/code mismatch, not an exploit — but it's the kind of thing a lottery integrator catches in prod and loses trust over. Verify against code, not README.

### [T3] `sdk/rust/src/cpi.rs:54-59` — no CU guard or explicit failure mode if consumer omits the 900K ComputeBudget ix

The CPI helper has no way to detect the tx's remaining CU budget. If the consumer forgot the `ComputeBudgetInstruction::set_compute_unit_limit(900_000)` (README §Compute Budget), the verify aborts mid-pairing with an opaque CU-exhaust error that does NOT map to any `AleaError`. Hardening: consider emitting a note in the `msg!` at verify entry like `msg!("alea: set CU>=900_000 if this tx fails")`. The TS SDK injects this automatically; Rust consumers have a sharp edge. Doc already calls it out strongly.

---

## Summary

Audit surface is tight. Three core guardrails — `seeds::program` on consumer side, `is_round_recent()` at consumer, immediate return-data capture — are correctly identified and mandated in README + rustdoc + ADR 0034. No T1 exploitable vulnerabilities found against a consumer following the mandatory constraints. Program-side trust boundaries are sound: Alea's `Verify` re-derives the Config PDA against `crate::ID`, blocking the fake-PDA shape at CPI time even if a consumer omits `seeds::program` (though omission still breaks consumer-local uses of `alea_config` for recency/game-state). `initialize` hardening via `UnauthorizedInit` + ProgramData upgrade-authority binding closes the deploy-to-init front-run. One notable recency gap (T2 future-round silent accept) narrows the anti-replay safety margin under the specific attacker who obtains unpublished drand sigs. One doc/code error-code mismatch (T3) will cost integrator trust. Otherwise: clean for a public-good drand verifier going into mainnet.
