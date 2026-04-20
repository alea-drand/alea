# Raul Mendoza — Web3 Client-Side Security Audit of `@alea-drand/sdk` v0.1.0

**Persona:** 6y appsec, wallet/dApp focus. 12 CVEs in Solana SDK ecosystem: keypair-in-stacktrace, no-timeout-fetch slow-loris, unsafe JSON.parse, seed-in-error-message, `Buffer.from(untrusted,'hex')` poisoning, wallet-adapter shape confusion.

**Scope:** `sdk/typescript/` + `README.md` + `CHANGELOG.md` + `LICENSE`. No Rust, no program, no CI.

**Threat model:** every external byte is hostile — drand API, signer object, RPC endpoint, user-supplied round. Looking for: injection, unvalidated deserialization, proto pollution, exception DoS, log/error leakage of secrets, timing oracles, unbounded retries, wallet phishing.

**Disposition:** SDK is small (~340 LOC across 6 files), clean, and free of the worst category of client-side footguns I routinely see. No `Buffer.from(untrusted,'hex')`, no `eval`, no dynamic `require`, no `JSON.parse` of external API bodies — `response.json()` is used, which also doesn't eval but does obey the Fetch spec. Signer never logged. Keypair never serialized. Error messages never include secret material. Retry loop is bounded. No regex run against attacker-controlled stdin except log lines the user's own RPC returned. Below: 2 T2, 5 T3, 1 informational. **No T1.**

---

## Findings

### [T2] sdk/typescript/src/drand.ts:49-55 — `hexToBytes` silently coerces non-hex characters to NaN then to 0

`parseInt("zz", 16)` returns `NaN`. `Uint8Array` coerces `NaN` to `0`. An attacker-controlled drand endpoint (MITM, DNS hijack, or a compromised endpoint in the 5-list) returning `signature: "zz..."` produces a **silent all-zeros signature** rather than rejecting the beacon. The on-chain verifier will then reject it as `InvalidG1Point` (6001) — so this is **not** a client-side exploit for randomness forgery (that's defeated by the pairing check). But it eats the user's tx fee (~0.000005 SOL) on a malformed upstream response and surfaces a confusing error. Also: odd-length `hex.length/2` rounds down silently → truncated buffer. **Fix:** validate `/^[0-9a-fA-F]+$/.test(hex) && hex.length % 2 === 0 && hex.length === 128` (signatures are exactly 128 chars) before the loop; throw `AleaError(0, "drand API returned malformed signature")`. Also validate `data.round === Number(targetRound)` to detect a rogue endpoint returning a different round than requested.

### [T2] sdk/typescript/src/drand.ts:58-101 — unbounded response size + silent `catch` swallows all errors

`await response.json()` has no size cap. A hostile endpoint (api.drand.sh MITM via cert pinning gap, or one of the 5 is compromised) can return a multi-gigabyte body and exhaust the user's tab memory / freeze the Node process. `AbortSignal.timeout(5000)` covers network stall but NOT slow-body-streaming once headers arrive within 5s. Fetch spec permits slow-loris on body. **Fix:** `response.headers.get('content-length')` check (reject > ~1KB; a drand beacon JSON is ~250 bytes) and/or read via `response.body` reader with an accumulated byte cap. Separately, `catch {}` at line 90 swallows `TypeError` from a malformed body — use `catch (e)` and rethrow non-network errors so a poison endpoint fails fast instead of silently rotating to the next. Worst-case retry budget is 77s (3×5×5s + delays) which is an information/DoS vector: an attacker controlling DNS for all 5 drand domains can hold the user's tab open for 77s per call. Consider exposing a `timeoutMs` option so a consumer can shorten this.

### [T3] sdk/typescript/src/client.ts:28-30 — `isBrowserWallet` discriminant trivially spoofable, but impact contained

`"sendTransaction" in signer` is a pure structural check. An attacker who already controls the `signer` argument has full compromise anyway (they ARE the signer). The discriminant only decides whether to wrap in `new anchor.Wallet(kp)` or use as-is. A Keypair with an attached `sendTransaction` property would skip wrapping and then crash at `wallet.signTransaction` (Keypair has no such method). No privilege escalation — just a confusing error. **Fix (defensive-depth):** use a stronger positive check — `typeof (signer as any).publicKey !== "undefined" && typeof (signer as any).signTransaction === "function"` — and prefer `instanceof Keypair` as the primary branch. Document in JSDoc that callers MUST pass either a `Keypair` or a wallet-adapter that conforms to the `Wallet` interface.

### [T3] sdk/typescript/src/client.ts:24 — `JSON.parse(readFileSync(idlPath))` at module import, synchronous, no integrity check

The IDL is bundled with the package (not fetched remotely), so supply-chain risk is pinned to npm tarball integrity — which npm publishes enforce via subresource hashes for the registry. A prototype-pollution payload in the IDL would have to ship in the published tarball, i.e. the attacker would need publish rights. `JSON.parse` itself does NOT install `__proto__` as a live prototype in modern Node (V8 treats it as own-property) — safe. **Hardening:** after parse, `Object.freeze(idl)` and assert `idl.address === DEVNET_PROGRAM_ID.toBase58()` to catch a tampered bundle. The `idlPath` resolution via `import.meta.url` is filesystem-race-safe (loaded once at module init, no TOCTOU).

### [T3] sdk/typescript/src/client.ts:96-100 — IDL `address` override lets any `programId` masquerade

`{ ...idl, address: programId.toBase58() }` patches the IDL's `address` field with whatever programId the caller passes. If a consumer is tricked into passing an attacker-controlled programId (e.g. from a malicious config file or phishing URL query param), Anchor will build a transaction targeting that program with the correct discriminator + args. The attacker's program could then accept any signature, emit `BeaconVerified`, and return 32 bytes of non-random data — defeating the entire trust model. This is **not** an SDK vulnerability per se (caller controls programId), but the SDK offers zero defense. **Fix:** maintain an allowlist of known-good program IDs (`DEVNET_PROGRAM_ID` + future `MAINNET_PROGRAM_ID`) and warn/throw when an unknown ID is passed without an explicit `{ allowCustomProgramId: true }` flag. At minimum, document this attack vector in CAVEATS.md §"Program ID Spoofing."

### [T3] sdk/typescript/src/client.ts:64-69 — `extractErrorCode` regex on RPC logs — no ReDoS, but input-length unbounded

`/Error Number: (\d+)/` and `/custom program error: 0x([0-9a-fA-F]+)/` are both linear regex (no nested quantifiers, no backtracking) — **no ReDoS**. Good. However, `logs: string[]` from `info.meta.logMessages` is attacker-influenceable: a malicious validator or MITM'd RPC could inject `"Error Number: 2001"` into a log line and trick the SDK into throwing `AleaError(2001, ...)` for a tx that actually succeeded or failed for a different reason. Impact: error-code confusion only — randomness bytes are only returned on `!info.meta.err`, so a success path cannot be spoofed into failure that looks like success. **Fix:** scan only program-log lines (`"Program <programId> invoke"` … `"Program <programId> success/failed"` blocks) rather than all logs, and require the programId to match.

### [T3] sdk/typescript/src/client.ts:134-137 — `skipPreflight: true` default with no opt-out

`skipPreflight: true` is hardcoded. Required because pairing outpaces the preflight blockhash window (documented). Downside: preflight is a *defense* for the user — it catches obviously-broken txs before committing fees. Malicious compute-budget ix or malformed signature now burns the fee on-chain. Also: `maxRetries: 3` at the RPC level combined with the 15-iteration `getTransaction` poll loop (1s sleep each = 15s) gives up to ~20s of tab-hold per verify call on top of the drand 77s. Total worst-case client-side time budget: **~97 seconds per `getVerifiedRandomness()` call**. **Fix:** expose `skipPreflight`, `maxRetries`, and a `totalTimeoutMs` as options; default `totalTimeoutMs: 30_000` and reject with a clear `AleaError` when exceeded — so consumer UIs can surface progress / cancel.

### [T3] sdk/typescript/src/instruction.ts:33-37 — `Buffer.alloc(8).writeBigUInt64LE(round)` throws on negative/oversized

`writeBigUInt64LE` throws `RangeError` for `round < 0n` or `round >= 2n ** 64n`. No try/catch in `createVerifyInstruction`. A consumer passing user-supplied `round` without bounds-checking will get an unhandled exception. Type system says `bigint` which includes negatives — but upstream `getCurrentRound()` / `fetchBeacon` always return positive. Still: `createVerifyInstruction` is a public API. **Fix:** `if (options.round < 0n || options.round >= 2n ** 64n) throw new AleaError(6002, "Round out of u64 range")` — reuse the existing 6002 `RoundZero` adjacency.

### Informational — non-issues confirmed safe

- **Wallet phishing (Q12):** a malicious wallet-adapter can return any `signTransaction` output it wants. The SDK then sends that signed tx — but the on-chain program re-derives `randomness = sha256(signature)` from the signature bytes the CALLER supplied in ix data. A malicious wallet that swaps the signed payload produces an invalid signature → `InvalidG1Point`. Cannot forge randomness. Cannot steal funds (tx is verify-only, no SOL transfer beyond tx fee to validator). **Safe.**
- **Error messages (Q4):** `AleaError` messages are static strings from the frozen `ERRORS` map — no pubkey, no RPC URL, no seed material ever interpolated. The one dynamic message (`ReturnDataMissing: ... for sig ${tx}`) leaks only the transaction signature, which is public on-chain. **Safe.**
- **Timing oracles (Q10):** no byte-by-byte comparisons on user-controlled secrets anywhere in the SDK. `createVerifyInstruction` serializes public data (round + signature + pubkey). **Safe.**
- **Prototype pollution via drand JSON (Q8):** `response.json()` in modern Node/browser does not install `__proto__` → V8 treats it as own-property. **Safe.**
- **`MAINNET_PROGRAM_ID` Proxy:** clever throw-on-access design; harmless to `await` / `typeof` / `String()` (returns undefined for those symbols). **Safe.**

---

## Summary for Aaron (under 180 words)

**Zero T1 client-side vulnerabilities.** The SDK avoids the categories I've found CVEs in across other Solana projects: no untrusted `Buffer.from(_,'hex')`, no keypair in logs, no seed in errors, no ReDoS, no synchronous `JSON.parse` of external bodies. Good hygiene overall.

**Two T2 hardening items** worth fixing before a paid audit or high-value integration:
1. `drand.ts:49-55` — `hexToBytes` silently zeros out non-hex chars. Validate shape (`/^[0-9a-f]{128}$/i`) before parsing. Also verify the returned `round` matches the requested one.
2. `drand.ts:58-101` — no response-body size cap; `catch {}` swallows parse errors. A MITM'd drand endpoint can hold the user's tab open for 77s or OOM it with a multi-GB response. Add content-length check and a `timeoutMs` option.

**Biggest defensive-depth gap:** `client.ts:96-100` — the IDL `address` is overridden by whatever `programId` the caller passes, with no allowlist. A phished consumer passing an attacker's programId gets attacker-controlled "randomness" with no SDK warning. Document in CAVEATS.md.

All other findings (T3) are polish — expose `skipPreflight`/`timeoutMs` options, narrow log-scan to Alea program lines, freeze the IDL object, bounds-check round in `createVerifyInstruction`.
