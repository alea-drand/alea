# Persona: Nia Okonkwo — TS Ecosystem Reviewer

**Role:** npm publishing readiness reviewer. 9 years TypeScript, @types team member, reviewer for packages with 10M+ weekly downloads.  
**Scope:** `sdk/typescript/` — package.json, tsconfig.json, src/**, tests/**  
**Date:** 2026-04-19  

## Execution Notes

Reviewed all source files, compiled dist output, `npm pack --dry-run` tarball manifest, release.yml and sdk-ts.yml CI workflows. No code was modified. Findings are ordered by severity within tier.

---

## Findings

### T1 — Publish Will Fail OR Tarball Is Broken

**[T1-A] `package.json` — `peerDependenciesMeta` entry for `@solana/wallet-adapter-base` has no corresponding `peerDependencies` entry**  
`sdk/typescript/package.json` line 57–61: `peerDependenciesMeta` declares `@solana/wallet-adapter-base` as optional, but `peerDependencies` is absent entirely from the manifest. `peerDependenciesMeta` is semantically meaningless without the base `peerDependencies` key — package managers (npm 7+, pnpm, yarn berry) will silently ignore the optional flag because there is nothing to flag as optional. A consumer importing the browser wallet path gets no install-time signal that they need `@solana/wallet-adapter-base`, which conflicts with the Browser Quick Start example in README.md. Fix: add `"peerDependencies": { "@solana/wallet-adapter-base": ">=0.15" }` alongside the existing `peerDependenciesMeta`.

**[T1-B] `client.ts` uses `fs`, `url`, `path` (Node built-ins) but README advertises browser support**  
`sdk/typescript/src/client.ts` lines 9–12: `readFileSync`, `fileURLToPath`, `dirname`, `join` are used to load the IDL at runtime — these are Node-only APIs. `dist/client.js` ships these imports verbatim. The README "Quick Start — Browser (User Pays)" example shows `import { getVerifiedRandomness } from "@alea/sdk"` inside a React component. Any bundler that doesn't polyfill `fs` (Vite, webpack default, esbuild) will fail at build time with "Cannot resolve module 'fs'". The package has `"type": "module"` and no `"browser"` field, no `"exports"` condition split (e.g., `"node"` vs `"browser"`), and no bundler shim. Fix: either remove the browser quickstart example and add a CAVEATS.md entry noting Node-only, OR extract IDL loading into a factory that accepts the parsed IDL as an argument and ship a separate browser-safe entry point.

---

### T2 — Real Ecosystem Hygiene Issues

**[T2-A] `peerDependenciesMeta` orphan causes `npm pkg fix` diff**  
`package.json` line 57: `npm pkg fix` will flag the orphaned `peerDependenciesMeta` key (no parent `peerDependencies`) as a structural anomaly in some npm versions. Pre-apply the fix before publish to avoid automated tooling noise and audit lint failures in consuming monorepos.

**[T2-B] `tsconfig.json` uses `moduleResolution: "bundler"` — breaks bare Node ESM consumers**  
`sdk/typescript/tsconfig.json` line 5: `moduleResolution: "bundler"` tells tsc to accept imports without extensions at the source level, but the compiled output (`dist/*.js`) still has explicit `.js` extensions in import specifiers (confirmed in `dist/index.js` and `dist/client.js`) because the source files use `.js` extensions explicitly. This combination is valid for the current code. However, `moduleResolution: "bundler"` also means consumers using TypeScript with `moduleResolution: "node16"` or `"nodenext"` and `allowImportingTsExtensions: false` may see type resolution errors when the package's `d.ts` files reference internal paths. The safer choice for a published library targeting broad Node compatibility is `moduleResolution: "node16"` or `"nodeNext"`, which is the standard recommendation for npm packages as of TS 5.x. Low urgency given the explicit `.js` extensions in source, but worth noting for ecosystem hygiene.

**[T2-C] Tarball ships IDL JSON twice — `src/idl/alea_verifier.json` and `dist/idl/alea_verifier.json`**  
`package.json` `files` array includes `"src/idl/**"`, and `prepublishOnly` copies `src/idl/*` into `dist/idl/`. The `npm pack --dry-run` confirms both paths land in the tarball (9.1KB each, 18.2KB duplicate). The `exports` map correctly resolves `./idl/alea_verifier.json` to `dist/idl/alea_verifier.json`. The `src/idl/` copy is dead weight — no consumer path resolves through it. Fix: remove `"src/idl/**"` from the `files` array; `dist/**` covers `dist/idl/alea_verifier.json` already. Saves ~9KB from every install.

**[T2-D] `MAINNET_PROGRAM_ID` types as `PublicKey` in `dist/constants.d.ts` but throws at runtime**  
`sdk/typescript/dist/constants.d.ts` line 6: `export declare const MAINNET_PROGRAM_ID: PublicKey`. At runtime it is a `Proxy` that throws on any property access. The type declaration is a lie — TypeScript consumers will see a valid `PublicKey` and pass it to `verifyDrandBeacon({ programId: MAINNET_PROGRAM_ID })` with no compile-time warning, then get an opaque runtime throw. Better option: type it as `never` or a branded opaque type (e.g., `{ readonly __mainnetNotSet: unique symbol }`) so the TS compiler rejects uses at call sites. The Proxy trap comment in source explains the intent but the `.d.ts` defeats it.

**[T2-E] `exports` map missing `"default"` condition — breaks some bundlers and Deno**  
`package.json` exports: `"."` has only `"types"` and `"import"`. Deno, some versions of esbuild, and older webpack resolve chains fall back to `"default"` when no matching condition is found before giving up. Adding `"default": "./dist/index.js"` as the final condition costs nothing and improves cross-runtime compatibility. This is standard practice for any ESM-only library in 2025.

**[T2-F] `release.yml` publishes npm without `--provenance` flag — provenance attestation absent**  
`.github/workflows/release.yml` line (npm publish step): `npm publish --access public` has no `--provenance` flag. npm 9.5+ supports `npm publish --provenance` which generates a Sigstore-signed SLSA provenance attestation, linking the package on the registry to the specific GitHub Actions run and commit. The workflow already runs on GitHub Actions with `actions/checkout@v4` and implicitly has OIDC token access, but `permissions: id-token: write` is not declared on the `release` job. Without it, `--provenance` will fail. This is a missed trust-signal for a cryptography SDK where supply-chain provenance is directly relevant to the security story. Fix: add `permissions: id-token: write` to the release job and append `--provenance` to the publish command.

**[T2-G] `devDependencies` pins `vitest: "^1.6"` — 1.x is EOL, currently at 2.x**  
`package.json` line 63: vitest 1.x reached end-of-life in early 2025; vitest 2.0 released April 2024. `^1.6` will never resolve to 2.x, so the test suite is locked to an unmaintained runner. No breaking API changes affect the current test patterns (vi.spyOn, describe/it/expect). Recommend `"vitest": "^2.1"` or `"^3.0"` (latest stable as of April 2026). Low risk to upgrade; high risk to leave on EOL tooling for a package going to npm.

---

### T3 — Polish

**[T3-A] Missing `"sideEffects": false`**  
`package.json`: no `sideEffects` field. For a library of pure utility functions, `"sideEffects": false` enables tree-shaking in webpack and Rollup — consumers importing only `fetchBeacon` won't pull in the entire Anchor/web3.js dependency graph. The `ERRORS` freeze and `MAINNET_PROGRAM_ID` Proxy in module-level code are the only side effects, and both are intentional. Either declare `false` (accepting the freeze/proxy execute on import, which they must anyway) or declare the specific files that have side effects.

**[T3-B] `engines.node: ">=18"` is slightly loose given `AbortSignal.timeout` usage**  
`sdk/typescript/src/drand.ts` line 26: `AbortSignal.timeout()` requires Node 17.3+. `">=18"` covers this and is the right LTS floor, but worth documenting explicitly that Node 16 (EOL Dec 2023) is not supported.

**[T3-C] `README.md` comment: `// programId defaults to mainnet` is incorrect**  
`sdk/typescript/README.md` line 87: the inline comment `// programId defaults to mainnet; override for devnet with programId` is wrong — `programId` actually defaults to `DEVNET_PROGRAM_ID` (see `client.ts` line 91, `constants.ts` line 18). This will confuse first users into assuming they need to pass a programId for devnet when in fact the default already is devnet. Suggest: `// programId defaults to DEVNET_PROGRAM_ID; pass explicitly for mainnet once deployed`.

**[T3-D] `createVerifyInstruction` signature in README differs from implementation**  
`sdk/typescript/README.md` API reference for `createVerifyInstruction` shows `payer` is missing from the documented signature. The actual implementation (`instruction.ts` line 19) requires `payer: PublicKey`. The docs omit it, which will cause a confusing runtime error for anyone copying the documented call pattern.

---

## Summary

Two T1 blockers: the `peerDependenciesMeta`/`peerDependencies` mismatch is an npm structural error that will silently ship broken peer install signals, and the Node-only `fs`/`url`/`path` imports contradict the browser quickstart in README — any non-Node bundler will fail. Five T2 hygiene issues cover the orphaned peer meta, `moduleResolution: "bundler"` tradeoffs, duplicate IDL in tarball, the misleading `MAINNET_PROGRAM_ID: PublicKey` type declaration, a missing `"default"` export condition, absent provenance attestation, and an EOL vitest pin. No T1 requires new source logic — all are configuration or documentation fixes.
