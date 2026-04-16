---
name: Feature Request
about: Suggest a new feature or SDK improvement
title: '[FEATURE] '
labels: enhancement
assignees: ''
---

## Problem

<!-- What problem does this solve? Who feels the pain? -->

## Proposed Solution

<!-- What are you asking for specifically? Sketch an API if relevant. -->

## Alternatives Considered

<!-- What else did you consider? Why is your proposal better? -->

## ADR 0028 Check (CPI Interface Stability)

Per Alea's CPI Interface Stability Guarantee (see public docs at https://alea.so/spec#cpi-stability — Phase 6 URL; pre-docs-site reference: `CHANGELOG.md` §"Versioning Policy"), new capabilities ship as new instructions — never as modifications to `verify` v1.

- [ ] My proposal is **additive** (new instruction, new SDK helper) — shipped as a minor version bump
- [ ] My proposal requires **breaking changes to `verify` v1** — I understand this is explicitly forbidden and would require a new mainnet program ID (not feasible as a PR to Alea v1)
- [ ] My proposal is **spec or documentation** only — no code interface change

## Additional Context

<!-- Anything else: links to related work, prior art, use cases. -->
