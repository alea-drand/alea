# Changelog

All notable changes to `@alea-drand/sdk` are documented here. Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [semver](https://semver.org/).

## [0.2.0] — 2026-04-20

### Added
- `verifyDrandBeaconWithMeta()` — variant of `verifyDrandBeacon` that returns `VerifyMeta` (randomness + Solana tx signature + slot + compute units + fee) instead of just the 32-byte randomness. Use when you need to link to Explorer, report CU usage, or compute per-call cost without a post-call `getSignaturesForAddress` query.
- `getVerifiedRandomnessWithMeta()` — high-level counterpart that fetches the drand beacon and verifies on-chain in one call, returning `{ round, signature, ...VerifyMeta }`.
- `VerifyMeta` type export — the on-chain metadata shape shared by both new functions.

### Changed
- `verifyDrandBeacon` is now a thin wrapper around `verifyDrandBeaconWithMeta`. Behavior and return type unchanged (still returns `Promise<Uint8Array>`).

### Internal
- Consolidated verify flow into a single implementation (`verifyDrandBeaconWithMeta`); `verifyDrandBeacon` discards the meta fields. No duplicate code paths — both exports share the same on-chain logic.

### Unchanged
- Peer dependency ranges (`@solana/web3.js ^1.95.0`, `@coral-xyz/anchor ^0.30.1`, `@solana/wallet-adapter-base ^0.9.0`) — new functions reuse existing code paths.
- Error codes, constants, IDL.
- Existing public API: `verifyDrandBeacon`, `getVerifiedRandomness`, `fetchBeacon`, `getCurrentRound`, `getRoundAt`, `isRoundRecent`, `createVerifyInstruction`, `getConfigAddress`, `AleaError`, `ERRORS`.

## [0.1.0] — 2026-04-19

Initial public release.

### Added
- `getVerifiedRandomness()` — high-level verify-in-one-call entry point.
- `verifyDrandBeacon()` — IDL-based submission for pre-fetched round + signature.
- `fetchBeacon()`, `getCurrentRound()`, `getRoundAt()`, `isRoundRecent()` — drand helpers.
- `createVerifyInstruction()`, `getConfigAddress()` — raw instruction builder + PDA helper.
- `AleaError` + `ERRORS` — typed error mapping for on-chain + SDK-side failures.
- Cluster-agnostic program ID (`DEVNET_PROGRAM_ID` === `MAINNET_PROGRAM_ID`).
- Anchor 0.30.1 + web3.js 1.98 incompat workaround (bypasses `.rpc()`, uses `.transaction()` + `signTransaction` + `sendRawTransaction`).
- 5-endpoint drand fallback with 3 retries and cross-endpoint round validation.
- Input validation + abort signal support + `skipPreflight` override.
