---
name: Bug Report
about: Report a bug in the Alea program or SDKs
title: '[BUG] '
labels: bug
assignees: ''
---

## Summary

<!-- One sentence describing the bug. -->

## Environment

- **Cluster:** <!-- mainnet-beta / devnet / localnet / testnet -->
- **Alea program ID used:** <!-- from docs site or README -->
- **SDK language:** <!-- Rust (alea-sdk) / TypeScript (@alea-drand/sdk) / raw -->
- **SDK version:** <!-- from `cargo.toml` or `package.json` -->
- **Anchor version:** <!-- from `anchor --version` -->
- **Rust version** (if applicable): <!-- `rustc --version` -->
- **Node version** (if applicable): <!-- `node --version` -->
- **Solana CLI version:** <!-- `solana --version` -->

## Steps to Reproduce

<!-- Minimum reproducible code snippet. Include:
     - What round + signature you called verify() with
     - Full tx signature (if on-chain) — or Solana Explorer link
     - Expected vs actual outcome
-->

```rust
// or typescript — paste minimal repro here
```

## Expected Behavior

<!-- What you expected to happen. Reference the spec file and line if relevant. -->

## Actual Behavior

<!-- What actually happened. Include full error message + stack trace. -->

## Program Logs

<!-- If on-chain: paste logs from `solana logs` or Solana Explorer.
     Pay attention to AleaError codes (6000-6009) and Anchor 2001 — see the
     full error code table at https://alea.so/errors (post-Phase-6 docs site).
     The on-chain program source `programs/alea-verifier/src/error.rs` is the
     canonical pre-docs-site reference. SECURITY.md does NOT document error
     codes — it covers the disclosure policy only.
-->

```
Program log: ...
```

## Additional Context

<!-- Anything else relevant: is this a regression from a prior version? Does it
     happen on all drand rounds or specific ones? Does it happen with a specific
     wallet adapter? -->

## Security?

- [ ] I have verified this is NOT a security vulnerability. Security issues go through `.github/SECURITY.md` (private channel), NOT this template.
