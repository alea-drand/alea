<!-- Thanks for contributing to Alea. Please fill out the checklist below. -->

## Summary

<!-- One or two sentences describing the change. -->

## Type of change

- [ ] Bug fix (non-breaking, patches an existing issue)
- [ ] New feature (non-breaking addition — new instruction, new SDK helper)
- [ ] Breaking change (requires major version bump)
- [ ] Documentation / spec update (no code change)
- [ ] CI / tooling

## CPI Interface Stability (ADR 0028)

- [ ] I confirm this PR does **NOT** modify an existing instruction signature (including `verify`, `initialize`, `update_config`)
- [ ] I confirm this PR does **NOT** modify the `Config` account layout
- [ ] I confirm this PR does **NOT** modify the `BeaconVerified` event schema
- [ ] If ANY box above is UNCHECKED: this PR requires an ADR amendment or is a v2 (new program ID) deployment, not an upgrade. See [ADR 0028](../build-spec/decisions/0028-cpi-versioning.md).

## Test coverage

- [ ] Existing tests pass (`anchor test` locally)
- [ ] New tests added for new behavior (if applicable)
- [ ] Test vectors in `testing/fixtures/` unchanged OR regenerated + committed (see [ADR 0029](../build-spec/decisions/0029-test-vectors-precommitted.md))

## Security (required for changes to `programs/alea-verifier/` or `sdk/`)

- [ ] No new `unsafe` blocks added (or justified inline)
- [ ] No changes to error codes (6000-6009) without ADR update
- [ ] No changes to `seeds::program` constraint in CPI examples (ADR 0034 — fake config defense)
- [ ] No reintroduction of `Unauthorized` custom error variant (T1.06)

## Spec alignment

- [ ] `build-spec/` updated if spec needs to change
- [ ] `CHANGELOG.md` Unreleased section updated

---

Closes # <!-- issue number if applicable -->
