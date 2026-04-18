pub mod initialize;
pub mod update_config;
pub mod verify;

// T1.04 — BPF-vs-native `map_to_point` parity debug instruction.
//
// SECURITY POSTURE: this instruction is INTENTIONALLY always present
// in the shipped binary. Anchor 0.30.1 has a known limitation that
// prevents cleanly cfg-gating a single instruction inside `#[program]`
// (the macro emits client bindings that don't get cfg-stripped, so
// feature-flagging this single fn causes E0432 unresolved-import
// errors in non-test builds).
//
// Risk analysis: `map_to_point_debug` is a STATELESS PURE FUNCTION.
//   - No accounts mutated (no Config writes, no Account<Config> access)
//   - No authority check (no `has_one`, no signer verification beyond
//     the standard fee-payer Signer required by every Solana tx)
//   - No side effects (emits no events, sets no state)
//   - Input is 32 bytes of field-element data; output is 64 bytes of
//     map_to_point(u). Same computation is available off-chain via any
//     SVDW reference (gnark-crypto, noble-bn254-drand).
//   - CU cost ~15-30K; each call costs the caller their own tx fee.
//   - Cannot be used to spoof randomness (returns raw map_to_point
//     output, NOT a BLS-verified beacon).
//
// In other words: this instruction exposes a deterministic pure
// function that anyone could compute off-chain. It adds zero attack
// surface — analogous to an `abs()` or `sqrt()` helper that happens
// to run on-chain. The test harness uses it to exercise the BPF code
// path with gnark-verified inputs to prove byte-for-byte parity (T1.04).
pub mod map_to_point_debug;
