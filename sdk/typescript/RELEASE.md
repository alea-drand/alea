# @alea-drand/sdk — Release playbook

Canonical step-by-step for cutting a new npm release of `@alea-drand/sdk`. Follow top to bottom; don't skip.

This file lives with the code because npm publish is operational: CI can't do it blind (2FA, signing, version-bump decisions) and neither should an agent. Future releases follow the same cadence even if the people involved change.

## Before you start

- **Branch**: you must be on a feature branch off `main`, not `main` itself. Name convention: `feature/sdk-<short-topic>` (e.g., `feature/sdk-verify-with-meta`).
- **Clean tree**: `git status` shows no uncommitted changes in `sdk/typescript/` other than the release work itself.
- **Tests green** on your feature branch: `npm test` passes locally before starting the release workflow.
- **npm auth**: you're logged in as a maintainer of `@alea-drand` (`npm whoami` → `aaronisafk`). 2FA device near you.
- **Clear scope** of what's in the release — draft the CHANGELOG entry mentally before touching files.

## Version decision (semver)

| Change | Bump |
|---|---|
| Backward-compatible bugfix, no API change | PATCH (`0.x.y → 0.x.y+1`) |
| Backward-compatible new function, type, or optional parameter | MINOR (`0.x.y → 0.x+1.0`) |
| Breaking change to existing export (signature, semantics, removed export) | MAJOR (`0.x.y → 0.y.0` pre-1.0; `x → x+1` post-1.0) |
| Peer dependency range tightens | MAJOR (consumers must re-check) |

For `0.x.y` releases, breaking changes are tolerated at MINOR boundaries per common pre-1.0 convention — but document them loudly in CHANGELOG. After 1.0, breaking changes MUST be MAJOR.

## Release flow

### 1 — Update `CHANGELOG.md`
Add a new top entry with the new version and today's date (`YYYY-MM-DD`, ISO). Categorize changes as:
- **Added** — new exports, new options
- **Changed** — behavior changes (backward-compat)
- **Deprecated** — marked-for-removal APIs
- **Removed** — deleted exports (breaking!)
- **Fixed** — bug fixes
- **Security** — vulnerability fixes
- **Internal** — refactors, no API change

List the **Unchanged** peer dependency ranges explicitly — consumers want to see this.

### 2 — Bump `package.json` version
Edit `"version"` to the new number. Do NOT run `npm version` — it tags prematurely and can skip CHANGELOG coordination.

### 3 — Update `README.md` if the public API changed
- New functions get a section in "API Reference".
- Deprecated functions get a `> DEPRECATED — removed in vX.Y.Z` callout.
- Version the callout: `### functionName(args) — since 0.X.Y`.

### 4 — Verify peer dependency ranges
`cat package.json | jq .peerDependencies` — compare against the previous release. If ranges tightened, CHANGELOG must flag it and bump MAJOR.

### 5 — Build + pack dry run
```bash
cd sdk/typescript
npm run build
npm test
npm pack --dry-run
```
The dry-run prints the tarball shape. Inspect:
- **Version** in tarball name matches `package.json`
- **Files** includes `dist/**`, `README.md`, `CAVEATS.md`, `LICENSE`, `CHANGELOG.md` (if `files` in package.json lists it)
- **`dist/index.d.ts`** contains all new type exports
- **`dist/index.js`** has no references to private/scratch code

### 6 — Devnet integration smoke
Run the gated devnet tests — catches any wire-level regressions that unit tests miss.
```bash
ALEA_DEVNET_TESTS=1 npm test
```
This burns ~0.0001 SOL devnet per test run. Zero cost, but validates the full on-chain flow.

### 7 — Commit the release on your feature branch
```bash
git add -A sdk/typescript/
git commit -m "feat(sdk): release @alea-drand/sdk@<version> — <short summary>"
```
One commit for the whole release change-set (CHANGELOG, package.json, README, src/, tests/). Keeps `git log` readable.

### 8 — Open PR to `main`
```bash
gh pr create --title "feat(sdk): v<version> — <summary>" --body "$(cat <<EOF
## Summary
<1-3 bullet points from CHANGELOG>

## Release checklist
- [x] CHANGELOG updated
- [x] package.json version bumped
- [x] README updated for any new APIs
- [x] npm test passes
- [x] ALEA_DEVNET_TESTS=1 npm test passes
- [x] npm pack --dry-run inspected

## Publish
Publish to npm after merge. See RELEASE.md step 9.
EOF
)"
```

### 9 — After PR merges: publish + tag
On `main` (pull first):
```bash
cd sdk/typescript
git checkout main && git pull
npm publish --access public
```
2FA prompt → authenticate. On success, npm prints the published version + SHA.

Verify:
```bash
npm view @alea-drand/sdk@<version>
```
Shows the new version metadata within ~30 seconds.

Tag the release:
```bash
git tag "sdk-typescript-v<version>" -m "@alea-drand/sdk@<version>"
git push origin "sdk-typescript-v<version>"
```

**Tag naming**: `sdk-typescript-v0.2.0` (NOT `v0.2.0` — that namespace is reserved for the protocol repo's audit tags like `v0.2.0-audit-passed`).

### 10 — Cut a GitHub Release
```bash
gh release create "sdk-typescript-v<version>" \
  --title "@alea-drand/sdk <version>" \
  --notes "$(sed -n '/^## \[<version>\]/,/^## \[/{/^## \[/d; p;}' sdk/typescript/CHANGELOG.md)"
```
Or use the GitHub UI if the CLI sed feels clumsy.

### 11 — Update downstream consumers
Any repo that depends on `@alea-drand/sdk` gets a bump PR:
- `alea-site/relayer/` → bump to `^<version>` in `package.json`
- Example consumers, docs examples, tutorials

This is NOT part of the release commit — it's post-release work so downstream adoption is traceable in its own PR.

## Failure modes + recovery

### npm publish fails with 403 / E_AUTH
- Confirm `npm whoami` shows your maintainer account.
- Confirm `@alea-drand` scope lists you as maintainer: `npm access list packages @alea-drand`.
- 2FA tokens expire — re-run `npm login` with `--auth-type=web`.

### npm publish fails with "version already exists"
- Someone already published this version (check `npm view @alea-drand/sdk versions`).
- Bump PATCH + republish. Never attempt `--force` or unpublish — unpublish has 72-hour grace but confuses lockfiles everywhere.

### Lockfile drift at top-level after dependabot merged
- `git pull origin main` first.
- Delete `package-lock.json` + `node_modules`, `npm install`, re-test.
- See [[2026-04-20-npm-lockfile-drift-after-dependabot]] in the vault for the full pattern.

### Tag already exists locally
- `git tag -d sdk-typescript-v<version>` to drop local.
- Never force-push tags to origin — use a fresh version bump instead.

### `npm pack --dry-run` shows missing files
- Check `"files"` in package.json — must include every path you need in the tarball.
- `.npmignore` or `.gitignore` can over-exclude; npm uses `.npmignore` if present, else `.gitignore`. Keep files list explicit in package.json rather than relying on ignores.

### Published package is broken (post-publish discovery)
- **Do NOT unpublish** — breaks consumers' lockfiles silently.
- Bump PATCH with a fix, publish again. Add a deprecation notice to the broken version via `npm deprecate @alea-drand/sdk@<broken-version> "broken, use @alea-drand/sdk@<fixed-version>"`.

## Related

- `~/.claude/rules/framework-gotchas.md` §Node.js/ESM — ESM export quirks across Node versions
- `~/vault/80-learnings/2026-04-20-alea-sdk-release-process.md` — rationale for each step + historical failure modes
- `~/vault/80-learnings/2026-04-20-npm-lockfile-drift-after-dependabot.md` — lockfile gotcha
- `~/vault/80-learnings/2026-04-19-node-json-import-syntax-fragmentation.md` — JSON import portability
- `~/vault/80-learnings/2026-04-19-solana-bpf-rustc-lag-external-consumers.md` — Rust SDK consumer pinning (relevant for sibling release process)
