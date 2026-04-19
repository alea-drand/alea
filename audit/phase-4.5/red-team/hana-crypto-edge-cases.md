# Hana Kobayashi — Crypto Edge-Case Red-Team

**Persona.** Cryptographer, PhD elliptic curve cryptography, 7y applied
research, arkworks contributor, BLS audit contributor. I reason at the curve
level — G1/G2 subgroups, pairings, SVDW branches, Montgomery form, sgn0.

**Scope.** `programs/alea-verifier/src/` crypto pipeline + `sdk/rust/` CPI
surface + `sdk/typescript/src/drand.ts` beacon shape. Code read only.

**Methodology.** Map the verify pipeline end-to-end, stress each primitive
against its failure modes (bad points, degenerate field elements, wrong
encodings, syscall oracle pathologies). For every probe: mathematical
scenario → expected behavior → actual code path → tier.

---

## Probe log

### 1. G1 point at infinity (identity)

**Scenario.** Attacker submits `signature = [0u8; 64]` encoding `(0, 0)`.
Pairing with identity = 1_GT → unguarded check would accept any round.

**Actual (`pairing.rs` on_curve_g1 + test `all_zero_bytes_is_rejected`):**
the on-curve equation `y² == x³ + 3` with `(0,0)` gives `0 == 3`, which is
false. Rejected at 6001 before pairing. BN254 uses the short Weierstrass form
with `B=3`, and the affine encoding has no distinguished "identity" byte
pattern — so `(0,0)` is correctly treated as off-curve. **Verified safe.**

### 2. G1/G2 subgroup membership

**G1.** BN254 G1 cofactor = 1; every on-curve point is in the prime-order
subgroup. `on_curve_g1` doc-block correctly cites
`is_in_correct_subgroup_assuming_on_curve -> true` and calls this out as
curve-specific. Not a vulnerability — but a future BN curve change would
silently break this assumption. Tier = T3.

**G2.** BN254 G2 cofactor ≠ 1; a malicious-but-on-curve G2 point in a small
subgroup would enable forgery. ADR 0027 "fallback path" uses byte-for-byte
equality `pubkey_g2 == EXPECTED_EVMNET_PUBKEY` instead of a runtime subgroup
check (the primary `is_in_correct_subgroup_assuming_on_curve` exceeds 1.4M
CU on BPF). This is **strictly stronger** than a subgroup check for the
single-chain deployment: the guard rejects any non-canonical pubkey, in-
subgroup or not. See `initialize.rs:95` + `update_config.rs:54`. The
`non_subgroup_g2_point_rejected_by_fallback_path` test in `constants.rs`
proves the fallback path rejects a concretely-constructed non-subgroup
point. **Verified safe under fallback semantics.**

### 3. SVDW `NoSquareRoot` reachability

`try_sqrt_curve(x=0)` returns `None` (explicit zero-x guard on both native
and BPF — closes T1.08 Agave short-circuit divergence, mirrored symmetrically
across `#[cfg(not(target_os="solana"))]` and `#[cfg(target_os="solana")]`
branches). `map_to_point` propagates `None` when all three x-candidates fail.
Per the BN254 SVDW theorem at least one candidate is a QR for every u, so
the `None` path signals constant corruption / syscall oracle regression,
not attacker-reachable input under honest drand. Error 6004 is wired
through `hash_round_to_g1` + `verify_beacon_full`; the test suite pins the
6004 code-mapping (`pairing_error_6006_code_mapping_stable` also asserts
6004). **Reachable only under infra fault — dead-code risk low.** Tier T3.

### 4. Round → message hash encoding

`hash_round_to_g1(round)` calls `round.to_be_bytes()` → `keccak256(8-byte BE)`.
Test `hash_round_to_g1_uses_big_endian_round_bytes` pins the 8-byte BE
convention against the gnark-verified fixture
`keccak256(00 00 00 00 00 00 00 01) = 6c31fc…100f`. No off-by-one: the 8
bytes are emitted by `u64::to_be_bytes()`, so length and encoding are
guaranteed by the standard library. **Verified safe.**

### 5. SVDW constants

`constants.rs` declares Z=1, A=0, B=3, C1=4, C2=-1/2, C3=sqrt(-12), C4=-16/3.
Three const sanity tests pin them: `c3_squared_equals_neg_12`,
`c3_sgn0_is_zero` (RFC 9380 §8.9.1 smaller-root convention — critical, a
silent swap to p-C3 would flip every tv5 sign and swap x1 ↔ x2),
`c2_equals_neg_half`, `c4_equals_neg_16_over_3`. `P_BIGINT` is pinned
against `<Fq as PrimeField>::MODULUS` (ground truth, not self-consistency).
**No inline TODO/FIXME observed. Verified safe.**

### 6. Pairing encoding (-M negation)

`negate_g1` computes `(x, p - y)` via `-Fq::from_be(y_bytes)` — the ark-ff
field negation, which handles the `y = 0` edge (negation of 0 is 0, so
negate of infinity-like-point is itself; but 6001 already rejected such
input before this runs). The pairing input buffer is assembled as
`σ || G2_gen || neg_m || pubkey` (384 bytes, 4-slot layout for
`e(σ, G2_gen) · e(-M, pubkey) == 1_GT`). Confirmed against EIP-197 byte
order. `verify_pairing` does a constant-time byte-equality against the
hard-coded `GT_ONE = [0u8; 31] || [1u8]` — tested via `gt_one_is_eip197_true`.
**Verified safe.**

### 7. `alt_bn128_g1_decompress([0u8; 32])` early-return

Known Agave short-circuit: syscall returns `Ok([0u8; 64])` on all-zero input.
Alea uses decompress as a sqrt oracle inside `try_sqrt_curve` (svdw.rs:101).
The explicit `if x.is_zero() { return None; }` guard at svdw.rs:117 closes
this. Native mirror (svdw.rs:93) added by POSTFIX-T2-01 to prevent
native/BPF divergence. No other call site of `alt_bn128_g1_decompress`
exists in the tree. **User signature bytes never reach decompress** — verify
path uses uncompressed 64-byte sigs exclusively via `on_curve_g1`. Verified safe.

### 8. Randomness = sha256(signature) vs keccak256

`verify_beacon_full` calls `solana_program::hash::hash(signature)` (sha256)
to compute the 32-byte output, per ADR 0036. Test
`randomness_is_sha256_not_keccak256` pins both the positive assertion (sha256
matches drand API fixture) and the negative (keccak256 does NOT match).
TS SDK's `DrandBeacon.unverifiedRandomness` field is populated from
drand API's `randomness` field (drand.ts:82), which is also sha256 per
drand's `bls-bn254-unchained-on-g1` scheme. `client.ts` extracts the
32-byte return data from `info.meta.returnData.data` and returns it without
comparing against `unverifiedRandomness` — the program's on-chain sha256
is the source of truth; the TS field is purely a pre-verify hint.
**Consistency verified; no silent mismatch path.**

### 9. `round == 0` rejection before expensive math

`verify_beacon_full` (instructions/verify.rs:52) explicitly documents guard
ordering as "load-bearing": round-zero check is the FIRST line, before
`on_curve_g1`, `hash_round_to_g1`, and pairing. DoS cost to an attacker
submitting round 0 is one compare + one log line. **Verified safe.**

### 10. Malformed 64-byte input that passes on_curve

Any on-curve (x,y) pair that isn't a valid drand signature will fail pairing
(error 6000). Probability of stumbling onto a valid sig for any specific
round by random sampling is ≈ 2⁻²⁵⁴ (the signature space is the prime-order
subgroup, size r ≈ 2²⁵⁴). Rejection path is clean: 6000 InvalidSignature.
Test `verify_on_curve_forgery_returns_6000_exact` pins this exactly.
**Verified safe.**

### 11. G1 subgroup assumption (future-proofing)

See probe 2. The cofactor=1 assumption is correct for BN254 but documented
in `on_curve_g1`'s doc-block, so a curve migration reviewer will see it.
Tier T3 hardening opportunity: add a one-line compile-time `static_assertions`
guard or a test pinning `<G1Projective as Group>::COFACTOR == 1` so
an accidental curve-crate bump can't silently invalidate the reasoning.

### 12. Determinism (replay idempotence)

Test `verify_same_round_twice_returns_identical_randomness` at
verify.rs:266 pins byte-identical randomness for the same (round, sig)
pair across two invocations. The verify pipeline is stateless (reads
`config.pubkey_g2` only), so determinism is structural. **Verified safe.**

### 13. chain_hash check cost

`chain_hash == EXPECTED_EVMNET_CHAIN_HASH` is only checked at
`initialize_handler` + `update_config_handler`, NOT at `verify`. Rationale:
the Config PDA is a singleton whose `chain_hash` was set at init and is
guarded by the byte-equality constraint on every update. At verify time,
config is trusted (ADR 0028 PDA-singleton + ADR 0034 `seeds::program`
enforcement on the consumer side). The only reachable 6007 path is on
initialize/update — both authority-gated. **Verified safe** for verify hot
path. **Tier T3** observation: T2.Y comment at verify.rs:48 explicitly
considered adding a verify-time `pubkey_g2 == EXPECTED` defense-in-depth
and decided against it (+200 CU unjustified) — reasonable, documented.

### 14. Config PDA pubkey substitution attack

The ONLY attack surface is a consumer program that forgets
`seeds::program = alea_program.key()` on their `alea_config` account. Without
it, an attacker passes a fake Config PDA from a different program-derived
address and controls `pubkey_g2`. ADR 0034 mandates the constraint; it's
re-emphasized in FOUR places: `sdk/rust/src/lib.rs:23-25` doc example,
`cpi.rs:38-40`, README, CAVEATS. The constraint is a **consumer-side**
requirement — alea-verifier cannot enforce it (the malicious PDA wouldn't
be owned by alea-verifier, so the `Account<Config>` deserialization would
fail, but only if the consumer actually typed `Account<alea_sdk::Config>`
rather than `AccountInfo` + manual parsing). **Verified: mandatory SDK
documentation present**; **Tier T2 observation:** an audit-tool lint rule
for downstream CPI consumers checking for the missing constraint is not
shipped — a motivated misuse could ship without it. Recommendation: add a
`compile_error!`-style macro helper (e.g. `alea_sdk::assert_config_seeds!`)
or a README lint example so mis-integrations surface loudly.

### 15. `alt_bn128_pairing` CU envelope

48,485 CU for 2 pairs per the documented per-call cost; `verify` full pipe
reports 454K CU under 900K budget (per `lib.rs` doc line 73-75). Malicious
input cannot inflate pairing CU — the syscall cost is fixed per pair count,
and input length is hard-coded to 384 bytes in `verify_pairing` (svdw.rs is
not user-reachable; only `verify_pairing` assembles the buffer and its
inputs are size-typed `[u8; 64]` / `[u8; 128]`). **Verified safe.**

---

## Additional finding: map_to_point_debug field reduction

`map_to_point_debug_handler` calls `fq_from_be_bytes(&u_bytes)` which uses
`Fq::from_be_bytes_mod_order` — input u ≥ p is silently reduced mod p.
This is correct for an SVDW input (u is defined over Fq, so reduction is
semantically equivalent), but it means the debug instruction does NOT
expose any way to test `u ≥ p` rejection. Not a vulnerability — map_to_point
is a pure function with no attack surface (see mod.rs SECURITY POSTURE).
Tier T3: if future instructions accept raw field-element input and MUST
reject non-canonical encodings, they need a `x < p` check analogous to
`on_curve_g1`'s canonical-form gate; the pattern is already in the codebase
(bytes_to_bigint + P_BIGINT compare at pairing.rs:40-46).

---

## Tiered findings

**T1 (cryptographic weakness, exploitable).** None.

**T2 (narrows safety margin).**
- **T2-HK-01.** Consumer-side `seeds::program` constraint is mandatory but
  enforcement lives in documentation only. A motivated mis-integration can
  ship without it; total compromise for that consumer. Recommendation: add
  a declarative macro helper (e.g. `alea_sdk::verified_config_pda!`) that
  emits the full `#[account(seeds=[b"config"], bump, seeds::program = …)]`
  block so the mandatory constraint is unavoidable at the syntax level.
  Defers out-of-scope for this red-team pass (SDK ergonomics), but flag for
  Phase 5 external audit review.

**T3 (hardening).**
- **T3-HK-01.** G1 cofactor=1 assumption in `on_curve_g1` is correct for
  BN254 but future-proofing recommends a compile-time or test-time pin of
  `<G1Projective as Group>::COFACTOR == 1` so a dep bump can't silently
  invalidate the subgroup-free path.
- **T3-HK-02.** `NoSquareRoot` (6004) is effectively unreachable under
  honest drand by SVDW theorem; the code path exists only for constant-
  corruption / syscall-oracle regression. Consider adding a differential
  BPF-vs-native parity test that exercises `map_to_point` across a
  statistically significant sample (e.g. 1000 rounds) to catch oracle drift.
- **T3-HK-03.** `map_to_point_debug_handler` silently reduces u ≥ p mod p.
  Document in the handler doc-block that inputs are reduced, not rejected,
  so future readers don't mistake it for a canonical-form validator.

## Summary

Crypto boundaries are tight. I probed 15 attack vectors (point at infinity,
G2 subgroup, SVDW NoSquareRoot reachability, round encoding, SVDW constants,
pairing negation, decompress zero short-circuit, sha256 vs keccak, round 0
ordering, malformed on-curve sigs, cofactor assumptions, replay determinism,
chain_hash cost, PDA substitution, CU envelope) and found zero T1 findings.
Guard ordering is explicitly documented as load-bearing and correct
(round>0 → on_curve_g1 canonical-form → hash_round_to_g1 → pairing). The
Agave `alt_bn128_g1_decompress([0u8;32])` short-circuit is guarded on both
native and BPF paths. ADR 0027 G2 fallback via byte-equality against the
hardcoded evmnet pubkey is strictly stronger than a runtime subgroup check
for the single-chain deployment. One T2 observation: the mandatory
consumer-side `seeds::program` constraint is documentation-only and could be
forgotten by a motivated mis-integration; recommend a declarative macro to
make the constraint unavoidable at the syntax level. Three T3 hardening
items on future-proofing assumptions. No critical crypto findings.
