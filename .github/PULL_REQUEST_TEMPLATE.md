<!-- Thanks for contributing to Alea. Please fill out the checklist below. -->

## Summary

<!-- One or two sentences describing the change. -->

## Type of change

- [ ] Bug fix (non-breaking, patches an existing issue)
- [ ] New feature (non-breaking addition — new instruction, new SDK helper)
- [ ] Breaking change (requires major version bump)
- [ ] Documentation update (no code change)
- [ ] CI / tooling

## CPI Interface Stability

The `verify` instruction signature, `Config` account layout, `Verify` Accounts struct, return-data format, and `BeaconVerified` event schema are **frozen forever** at the mainnet program ID. See README §Governance and CONTRIBUTING.md §Versioning Policy.

- [ ] I confirm this PR does **NOT** modify an existing instruction signature (including `verify`, `initialize`, `update_config`)
- [ ] I confirm this PR does **NOT** modify the `Config` account layout
- [ ] I confirm this PR does **NOT** modify the `BeaconVerified` event schema
- [ ] If ANY box above is UNCHECKED: this is a v2 (new program ID) deployment, not an upgrade. Open an RFC issue first.

## Test coverage

- [ ] `cargo test --workspace --lib --tests` passes locally
- [ ] `cd sdk/typescript && npm test` passes locally
- [ ] New behavior gets new tests (unit + devnet integration where applicable)

## Security

(required for changes to `programs/alea-verifier/` or `sdk/`)

- [ ] No new `unsafe` blocks added (or justified inline with a comment)
- [ ] No changes to error codes (6000–6012 reserved per versioning policy)
- [ ] No changes to `seeds::program` constraint in CPI examples (fake-config defense)
- [ ] No reintroduction of removed error variants

## Documentation

- [ ] `CHANGELOG.md` `[Unreleased]` section updated with a user-facing note
- [ ] `README.md` / `sdk/*/README.md` updated if public API or usage changed
- [ ] `sdk/rust/CAVEATS.md` / `sdk/typescript/CAVEATS.md` updated if maturity state changed

---

Closes # <!-- issue number if applicable -->
