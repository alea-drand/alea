# Red-Team Report: Replay, Griefing, and Front-Running Against Alea + example-lottery

**Persona:** Marcus Finley — Solana MEV researcher, consumer-layer attack specialization
**Scope:** example-lottery consumer, alea-verifier on-chain, alea-sdk (Rust + TypeScript)
**Date:** 2026-04-19
**Branch:** feature/phase-4-sdk

---

## Methodology

Attack surface was treated as external — no forge of drand signatures (cryptographically
infeasible: BLS threshold over 23 orgs). All attacks operate from the outside: choosing
WHICH round to submit, WHEN to submit it, WHO signs, and HOW to compose CPI chains.
Code was read; no changes were made.

---

## Attack Log

### A1 — Front-running resolve_bet via permissionless resolver

**Scenario:** MEV searcher watches mempool for `resolve_bet` txs, sees round `R` being
submitted, and races their own `resolve_bet` for a different round `R'` that gives a
favorable outcome.

**Code evidence:**
`resolve_bet` requires `player: Signer` AND `address = bet.player` constraint. Only the
original bettor can call `resolve_bet` for their own Bet PDA. An MEV searcher cannot
impersonate the player — they cannot produce the player's signature.

**Feasibility:** Not feasible. Player-signer constraint is structurally enforced.
**Severity:** T3 (non-issue — design eliminates attack class).

---

### A2 — Round pre-selection: observe drand before commit, choose favorable round as min_resolution_round

**Scenario:** Player fetches drand round `R` from an archive endpoint before committing.
They already know `sha256(sig_R)` = the randomness. They commit with `min_resolution_round = R`
and immediately resolve using that known round.

**Code evidence:**
`commit_bet` computes:
```rust
let min_allowed_ts = current_ts.saturating_add(alea_config.period);
let min_allowed_round = min_allowed_ts
    .saturating_sub(alea_config.genesis_time)
    .saturating_div(alea_config.period)
    .saturating_add(1);
require!(min_resolution_round >= min_allowed_round, GameError::MinRoundInPast);
```
With `period = 3s`, the floor is at minimum one full period in the future. Any round the
player can observe at commit time is, by definition, already emitted and will fail this
check. The player must commit to a round that has not yet been produced.

**Feasibility:** Not feasible. The `+period` floor ensures the resolver round postdates
the commit by at least one emission cycle.
**Severity:** T3 (correctly mitigated).

---

### A3 — Stale-round replay: submit a known-outcome round from hours ago

**Scenario:** Attacker archives drand round `R` from 2 hours ago (outcome known). They
call `resolve_bet` with `round = R` against a fresh bet.

**Code evidence — Guard 1:** `round >= bet.min_resolution_round` — since `min_resolution_round`
is forced to a future round at commit time (A2 above), any historical round trivially fails
this check unless the player committed with a `min_resolution_round` in the past, which is
also blocked.

**Code evidence — Guard 2 (independent):** `is_round_recent(..., MAX_BEACON_AGE_SECONDS = 30)`
rejects any round whose emission timestamp is more than 30 seconds before `Clock.unix_timestamp`.

Both guards independently reject hours-old rounds. Attacker needs both to fail simultaneously.
`is_round_recent` operates on `genesis_time` and `period` from the Config PDA — these are
byte-equality-hardcoded at init time (6010/6011 guards), so the Config cannot be poisoned
to fake recency.

**Feasibility:** Not feasible. Dual-guard architecture is correctly layered.
**Severity:** T3.

---

### A4 — Multiple bets sharing one drand round (correlated randomness)

**Scenario:** Two players both commit bets with `min_resolution_round = R`. Both resolve
against round `R`. They coordinate to bet opposite sides, guaranteeing one collects.

**Code evidence:**
The Bet PDA seeds are `[b"bet", player.key(), slot.to_le_bytes()]`. There is no constraint
preventing two distinct Bet PDAs from resolving against the same `round`. If two coordinated
players share the randomness, one wins and one loses — net house edge is zero for that pair,
but no player can guarantee a win.

**Actual impact:** This is a wash — identical randomness used for two opposite bets has
zero combined EV gain (the game is already 50/50). However, if this lottery had asymmetric
payouts or a house advantage, correlated round selection could reduce expected house edge
to zero. In the current implementation with 50/50 payout and no rake, there is no economic
incentive to exploit this.

**Deeper issue:** For higher-stakes consumers (e.g., Palestra), this round-correlation
attack matters more. The SDK `CAVEATS.md` and `lib.rs` docs explicitly note that consumers
are responsible for their own anti-collusion logic. example-lottery does not document this
as a known limitation.

**Feasibility:** Technically feasible, economically neutral in this implementation.
**Severity:** T2 — Not exploitable here, but the missing documentation warning is a
consumer-guidance gap. Future integrators could build an asymmetric lottery and miss this.

---

### A5 — Clock skew at the is_round_recent boundary

**Scenario:** `Clock.unix_timestamp` can drift from wall time by up to ~400ms on Solana
mainnet under normal conditions; under adversarial leader conditions, the leader can
skew slot timestamps within Solana's consensus rules (bounded by adjacent-slot timestamps).
Can an attacker push a stale round through by exploiting a skewed clock?

**Code evidence:**
`is_round_recent` uses `clock.unix_timestamp as u64` directly. The window is 30 seconds.
Maximum observable clock skew under consensus rules is on the order of seconds — insufficient
to move a rounds-old beacon into the 30-second window. The 30s window was chosen to tolerate
legitimate clock variance; tightening to 3s (one period) would harden this edge.

**Feasibility:** Extremely low — would require Solana consensus-level timestamp manipulation
well beyond what any single leader controls. Not a practical attack.
**Severity:** T3 (theoretical, practical infeasibility).

---

### A6 — Payer substitution / house-wallet capture

**Scenario:** Mallory pays for Alice's `resolve_bet` (as `payer`) and thus acts as "house"
on a loss. Can Mallory predict outcomes and only pay for resolve when Alice is about to lose?

**Code evidence:**
`player` must sign AND `address = bet.player`. `payer` is a separate signer and receives
the locked SOL on player loss. In a self-play configuration (`player == payer`), this is a
non-issue. In a custodial configuration where `payer` is a house wallet, the payer already
knows the round outcome before submitting (they control round selection within the
`is_round_recent` window). However, the `player` must also sign — the house cannot submit
without the player's signature, so the house cannot selectively resolve only losing rounds
against non-consenting players.

**Feasibility:** Not exploitable against a player who controls their own signing key.
Custodial designs where the house also holds the player's signing authority are outside
example-lottery's scope and inherently trust-based.
**Severity:** T3 (non-issue for self-custody design; custodial design is out of scope).

---

### A7 — Re-submit verified beacon to drain Config PDA rent

**Scenario:** Attacker calls `verify` repeatedly with the same round hoping to drain the
Config PDA's rent.

**Code evidence:**
`verify`'s account struct:
```rust
pub struct Verify<'info> {
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    pub payer: Signer<'info>,
}
```
`config` is read-only (no `mut`). No lamports move out of Config. The `payer` pays the tx
fee each time. Config is never debited. Repeated calls only drain the attacker's own wallet.

**Feasibility:** Not feasible as an attack on Config. Attacker self-harms only.
**Severity:** T3.

---

### A8 — Submit on-curve but cryptographically invalid signature

**Scenario:** Attacker constructs a 64-byte value that IS a valid G1 point (passes
`on_curve_g1`) but is NOT a valid drand signature for any round.

**Code evidence:**
The guard order in `verify_beacon_full` is:
1. `round > 0`
2. `on_curve_g1(signature)` — curve equation check
3. `hash_round_to_g1(round)` — deterministic, no attacker input
4. `verify_pairing(...)` — e(σ, G2_gen) == e(M, pubkey)

An on-curve forgery passes guard 2 but fails guard 4 with `AleaError::InvalidSignature`
(6000). The test `verify_on_curve_forgery_returns_6000_exact` explicitly pins this path.
Brute-force probability of a valid forgery: 2^-256 (discrete log hardness over BN254).

**Feasibility:** Not feasible. Pairing check is the cryptographic backstop.
**Severity:** T3.

---

### A9 — CU starvation via low compute budget

**Scenario:** Submit `verify` with `ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 })`
(Solana default). Alea needs ~454K CU baseline. Transaction fails, no state change. Does
this grief other users?

**Code evidence:**
A tx that runs out of CUs fails atomically — no side effects. Alea is stateless (no writes
on `verify`). CU starvation affects only the attacker's own tx fee. It cannot grief other
users' transactions because each Solana tx has its own isolated CU budget.

**Feasibility:** Not a griefing vector. Each tx fails in isolation.
**Severity:** T3.

---

### A10 — Two consumers CPI verify in same tx; second reads first's return data

**Scenario:** Tx contains: [CPI to Alea from Consumer A] then [CPI to Alea from Consumer B].
Consumer B's code reads Alea's return data — does it get Consumer A's randomness?

**Code evidence:**
Solana's return data is last-writer-wins within a transaction. Consumer B's CPI to Alea
overwrites Consumer A's return data. Consumer B correctly gets its own randomness. The
dangerous case is the REVERSE: Consumer A does a downstream CPI (e.g., token transfer)
AFTER the verify CPI but BEFORE capturing return data — that downstream CPI overwrites
Alea's return data.

**example-lottery code:** `let randomness = alea_sdk::cpi::verify(...)` is captured
immediately. The comment at line 133 explicitly flags this invariant. The SOL transfers
occur AFTER the capture. Correctly implemented.

**Risk to naive consumers:** The SDK `cpi.rs` doc contains a WRONG/CORRECT example
explicitly warning of this footgun. The risk is real for consumers who ignore the docs.

**Feasibility:** Not exploitable against example-lottery. Potential T2 issue for
future consumers who do not read SDK docs.
**Severity:** T2 — documentation footgun, not a protocol bug.

---

### A11 — MEV race via skipPreflight + blockhash expiration

**Scenario:** Attacker watches the mempool for `resolve_bet` transactions submitted with
`skipPreflight: true`. Attacker front-runs with a competing tx that consumes the target
slot's block space, racing blockhash expiration (~150 slots, ~60s).

**Code evidence (`client.ts`):**
The TypeScript SDK submits Alea's `verify` (not `resolve_bet`) with `skipPreflight: true`.
The `resolve_bet` instruction is on the consumer program (example-lottery), not the SDK.
For `skipPreflight: true` on `verify`, the concern is that an MEV searcher could pack
compute-heavy transactions in front to delay finalization past the blockhash window.

**Actual impact:** `verify` is purely additive — it emits a `BeaconVerified` event and
returns randomness. Racing `verify` has no EV for an attacker because the randomness output
is determined by the drand signature, not by who submits first. A delayed `verify` simply
expires and the user resubmits.

**Feasibility:** MEV on `verify` has zero EV for an attacker. The user's tx expires
harmlessly. `resolve_bet` requires the player's signature, so it cannot be front-run by a
third party.
**Severity:** T3.

---

### A12 — Malicious tx pre-verify to corrupt compute budget

**Scenario:** Attacker submits a tx that sets a high `ComputeBudgetProgram.setComputeUnitPrice`
just before the user's verify tx, making the user's tx unprofitable for validators and
causing it to be dropped.

**Code evidence:**
Compute unit price and limit are per-transaction parameters. A higher-priority fee tx from
an attacker does not corrupt or cancel the user's tx — it may delay it by occupying block
space, but Solana's fee market is additive, not destructive. The user's tx with its own
compute budget instruction is self-contained.

**Feasibility:** Fee griefing via higher-priced competing txs exists on all blockchains but
does not invalidate or corrupt the victim's tx. User resubmits with higher priority fee.
**Severity:** T3 (standard blockchain fee market behavior, not Alea-specific).

---

### A13 — maxRetries: 3 double-spend via re-submission

**Scenario:** `sendRawTransaction` uses `maxRetries: 3`. If the first submission succeeded
but the client's network layer timed out before receiving the signature, the client retries.
Does the retry cause a double-spend?

**Code evidence (`client.ts`, line 134-137):**
`verify` is a stateless instruction — it writes no on-chain accounts. Submitting the same
`verify` tx twice is idempotent in effect (both succeed; the second is a no-op replay at
the Solana level because the first already consumed that blockhash signature slot). More
importantly, a `verify` tx with a duplicate nonce (same recent blockhash + same signature)
is rejected by Solana's deduplication. The `maxRetries` is for UDP packet-level retransmit,
not application-level resubmission with a new blockhash.

For `resolve_bet`: the player controls submission independently. There is no SDK-managed
retry in example-lottery's flow. Even if retried, the Bet PDA is closed (`close = player`)
on successful resolution — a second call would fail with account-not-found.

**Feasibility:** Not a double-spend risk. Verify is stateless; resolve_bet closes the PDA.
**Severity:** T3.

---

## Tiered Findings Summary

| ID  | Attack                             | Severity | Exploitable | Notes                                      |
|-----|------------------------------------|----------|-------------|---------------------------------------------|
| A1  | Front-run resolve_bet             | T3       | No          | Player signer constraint is structural      |
| A2  | Pre-observed round at commit      | T3       | No          | +period floor enforced                      |
| A3  | Stale-round replay                | T3       | No          | Dual-guard: min_round + is_round_recent     |
| A4  | Correlated randomness across bets | **T2**   | Neutral EV  | Doc gap for asymmetric-payout consumers     |
| A5  | Clock skew at recency boundary    | T3       | No          | 30s window >> max consensus skew            |
| A6  | Payer substitution / house bias   | T3       | No          | Player must sign; custodial OOS             |
| A7  | Config PDA rent drain             | T3       | No          | Config is read-only in verify               |
| A8  | On-curve forgery                  | T3       | No          | 2^-256 via BN254 DL hardness               |
| A9  | CU starvation DoS                 | T3       | No          | Isolated tx failure, attacker self-harms    |
| A10 | Return-data ordering footgun      | **T2**   | Doc gap     | Correct in example-lottery; risky for naive consumers |
| A11 | MEV race via skipPreflight        | T3       | No          | Verify has zero EV for attacker             |
| A12 | Compute budget corruption         | T3       | No          | Fee market, not tx invalidation             |
| A13 | maxRetries double-spend           | T3       | No          | Verify stateless; resolve closes PDA        |

**T1 findings:** 0
**T2 findings:** 2 (A4, A10) — documentation gaps, not protocol bugs
**T3 findings:** 11 — correctly mitigated or structurally impossible

---

## Recommendations

**A4 (T2):** Add a note to `programs/example-lottery/src/lib.rs` (and SDK consumer docs)
stating that multiple bets can share the same resolution round and that consumers with
asymmetric payouts must enforce per-round bet limits at the application layer if correlated
randomness is a concern.

**A10 (T2):** The SDK `cpi.rs` already documents the return-data ordering footgun well.
Consider adding a lint or compile-time assert that captures the return value of
`alea_sdk::cpi::verify` immediately (e.g., via the `#[must_use]` attribute on the return
type) to make ignoring it a compiler warning rather than a runtime bug.

---

## Conclusion

The core protocol is sound against all replay, griefing, and front-running attacks
examined. The three-layer defense (player-signer requirement, min_resolution_round commit
floor, is_round_recent staleness window) is correctly composed and individually
load-bearing. No T1 findings. Two T2 documentation gaps that do not affect
example-lottery but could mislead future integrators building asymmetric games.
