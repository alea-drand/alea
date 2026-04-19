# Red-Team: Hostile drand Endpoint Analysis

**Persona:** Fen Lindqvist — network security researcher, supply-chain attacks on HTTP APIs  
**Date:** 2026-04-19  
**Scope:** `@alea/sdk` TypeScript client — `drand.ts`, `client.ts`, `constants.ts`, `errors.ts`  
**Out of scope:** on-chain program, Rust CPI crate, build-spec, vault

---

## Threat Model

A single entry in `DRAND_ENDPOINTS` is compromised (domain expiry + squatting, BGP hijack, DNS poisoning, CDN key exfiltration, TLS MITM via rogue CA). The adversary can serve arbitrary HTTP responses from that endpoint. The on-chain BLS pairing check is the cryptographic backstop — but it fires AFTER the user has paid a tx fee (~0.000005 SOL) and consumed up to 900K CU.

---

## Methodology

Line-by-line read of `fetchBeacon` and `getVerifiedRandomness`. Each probe asks: what does the compromised endpoint send, and what code path does the SDK execute?

---

## Probe Log

### Probe 1 — Slow-Loris (response stall after headers)

**Attack:** Endpoint sends `HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n` and then stalls, never sending the body.

**Code path:** `fetch(url, { signal: AbortSignal.timeout(5000) })` — the timeout fires at 5 s. `response.json()` is where the stall actually bites; `AbortSignal.timeout` is attached to the `fetch` call, not the body-read promise. In Fetch API spec, `AbortSignal.timeout` cancels the entire fetch including body streaming if it hasn't resolved. Node 18+ and all modern browsers implement this correctly. The 5 s clock starts at `fetch()` invocation.

**Outer loop:** `MAX_RETRIES=3`, `DRAND_ENDPOINTS.length=5`. Worst case with all 5 endpoints stalling on all 3 retries: 3 × 5 × 5 s + 2 × 1 s inter-attempt delays = **77 s**. The comment on drand.ts:57 documents this but there is no aggregate deadline across the entire `fetchBeacon` call. A caller has no way to impose a shorter global cap without wrapping in its own `Promise.race`.

**Verdict:** `AbortSignal.timeout` is correct per spec. The per-attempt timeout is sound. The macro exposure is a 77 s worst-case hang that cannot be cancelled by the caller without external wrapper — this is a DoS-of-convenience, not a security break.

**Severity: T3** — UX degradation only; no funds at risk. No cryptographic bypass.

---

### Probe 2 — Wrong-Round Spoofing

**Attack:** Endpoint returns `{"round": 99999, "signature": "<valid sig for round 99999>", "randomness": "..."}` for a request for round 100000.

**Code path:** `fetchBeacon` constructs URL with `targetRound.toString()` but accepts `data.round` from the JSON response without cross-checking against `targetRound`. The returned `DrandBeacon.round` is set to `BigInt(data.round)` — the attacker-controlled value. `getVerifiedRandomness` then passes `beacon.round` to `verifyDrandBeacon`. On-chain, the program computes `msg_hash = keccak256(round_as_u64_be)` using the round from the transaction argument — which is now 99999 instead of 100000. The signature is valid for round 99999 so **the BLS pairing check PASSES**. The caller receives verified randomness for a different round than they requested.

**Impact nuance:** If the consumer calls `getVerifiedRandomness({ round: undefined })`, they requested the latest round but received an older one. `is_round_recent` is a consumer-side check — the SDK does not call it. If the consumer's `max_age_seconds` is loose (e.g. 60 s), a round-spoofed beacon that is, say, 45 s old passes both the BLS check and the recency gate. The consumer's game-resolution runs on knowable (potentially pre-computed by attacker) randomness.

**Severity: T1** — Cryptographically silent substitution. Consumer receives valid randomness for a stale round they did not request. Exploitable if consumer's recency window is wider than one period (3 s). The SDK must cross-check `data.round === targetRound` after a successful response.

---

### Probe 3 — Replay of Known-Randomness Past Round

**Attack:** Variant of Probe 2 where the attacker replays a round whose randomness is publicly known and favorable to them (e.g. attacker's lottery ticket wins).

**Code path:** Same as Probe 2 — round is accepted verbatim. If `data.round` is old enough that `is_round_recent` would reject it, the exploit only works if the consumer omits that check. The SDK itself does not call `isRoundRecent` before submitting. `unverifiedRandomness` is returned alongside the beacon; the SDK does not warn if it is stale.

**Severity: T1** (shared root cause with Probe 2). The fix is the same: validate `data.round === targetRound` in `fetchBeacon`, and optionally add a staleness warning on `DrandBeacon.unverifiedRandomness` when `data.round` diverges from `getCurrentRound()` by more than one period.

---

### Probe 4 — Truncated / Malformed JSON

**Attack:** Endpoint returns `{"round": 12345, "signature": "abc"` (missing closing brace and `randomness` field).

**Code path:** `response.ok` is `true` (status 200), so the code reaches `await response.json()`. `JSON.parse` on a truncated body throws a `SyntaxError`. This lands in the `catch {}` block on drand.ts:90 — silently swallowed, `continue` to next endpoint. No error is surfaced to the caller until all retries are exhausted, at which point `AleaError(0, "All drand endpoints failed after retries")` is thrown.

**Verdict:** Graceful degradation — endpoint cycling is correct. Error message `code: 0` is somewhat opaque but sufficient for diagnosis.

**Severity: T3** — Correct handling, minor ergonomics gap on error code (0 is ambiguous).

---

### Probe 5 — Oversized Response

**Attack:** Endpoint streams a multi-hundred-MB body before the JSON closing brace.

**Code path:** `response.json()` buffers the full body before parsing. There is no `Content-Length` cap or `response.body` stream consumption limit. A 500 MB response would be buffered entirely in Node.js heap before `JSON.parse` fires. On a browser, `fetch` also buffers. Memory pressure can trigger OOM kill of the Node process.

**Severity: T2** — Requires a compromised or squatted endpoint to pull off. The impact is process crash (DoS), not cryptographic bypass. Mitigated partially by the 5 s `AbortSignal.timeout`: if the attacker trickles bytes slowly enough to keep the connection alive for 5 s while sending a large body, the abort fires and cuts it. If the endpoint streams 500 MB in < 5 s (LAN-rate: trivial), the buffer fills before abort.

---

### Probe 6 — Redirect to Malicious Host

**Attack:** Endpoint responds with `302 Location: https://evil.example/beacon`.

**Code path:** `fetch` follows redirects by default (no `redirect: "error"` or `redirect: "manual"` in the options). The redirect leads to an attacker-controlled server. If TLS is valid for `evil.example`, the fetch succeeds.

**`https://` prefix concern:** the endpoints are hardcoded in `constants.ts` as `https://` strings. A redirect to `http://` is an upgrade downgrade, which browsers block but Node 18+ follows silently in some implementations. A redirect to `file://` or `data:` would likely fail or be rejected by the Fetch API. The primary exposure is to a valid `https://` redirect target.

**Severity: T2** — Adding `redirect: "manual"` or `redirect: "error"` to the `fetch` call would close this. Without it, a compromised CDN (e.g. Cloudflare edge for `drand.cloudflare.com`) could silently redirect to an attacker-controlled JSON server.

---

### Probe 7 — Wrong chain_hash (Different drand Chain)

**Attack:** Endpoint returns a valid beacon from a completely different drand network (different chain hash, different G2 pubkey). BLS signature is valid for that chain's keypair but not Alea's stored pubkey.

**Code path:** `fetchBeacon` does not read `chain_hash` from the response body. The drand API embeds `chain_hash` in the URL path: `${endpoint}/${chainHash}/public/${round}`. A compliant server returns the beacon for the chain in the path. A hostile server ignores the path and returns whatever it wants. `fetchBeacon` accepts the response if `response.ok`.

**On-chain outcome:** The verifier checks `chain_hash` in the Config PDA against `EXPECTED_EVMNET_CHAIN_HASH` — but this check is about which chain's *G2 key* is stored, not a per-request chain_hash validation. The actual BLS pairing will fail (`InvalidSignature`, error 6000) because the signature was made with a different G2 key than the one in Config. The user pays tx fee + CU cost to learn this.

**SDK behavior:** No client-side chain verification. The error surfaces as `AleaError(6000, "InvalidSignature")` after the tx is submitted and confirmed — costing ~0.000005 SOL.

**Severity: T2** — Financial impact is trivial (micropayment per failure). Persistent endpoint serving wrong chain beacons causes repeated tx failures with no SDK-level explanation pointing to the root cause (wrong chain). Adding response-body `chain_info.hash` validation would make this T3.

---

### Probe 8 — BGP/DNS Takeover + Fallback Behavior

**Attack:** Attacker owns DNS for `api2.drand.sh`. TLS fails (cert mismatch) or they serve garbage.

**Code path:** TLS handshake failure or cert error causes `fetch` to throw. The `catch {}` silently swallows it, and the loop continues to `api3.drand.sh`, then `drand.cloudflare.com`, then `api.drand.secureweb3.com:6875`. Fallback is fully automatic with no logging. There is no mechanism for the caller to detect that a primary endpoint was skipped.

**Verdict:** Correct behavior — the multi-endpoint design is specifically the mitigation. TLS pinning is not in scope for a public-good library.

**Severity: T3** — No observable security gap; silent fallback is intentional.

---

### Probe 9 — All 5 Endpoints Compromised (Total Network Capture)

**Attack:** Attacker controls all 5 endpoints and serves BLS-invalid beacons consistently.

**Code path:** Every attempt returns `response.ok`, `response.json()` succeeds, a `DrandBeacon` is returned. `verifyDrandBeacon` submits the tx. On-chain pairing fails with `InvalidSignature` (6000). `AleaError(6000, ...)` is thrown. Consumer receives an error rather than randomness.

**Impact:** Service denial. Attacker cannot produce valid randomness — the BLS pairing is the cryptographic backstop and requires compromise of at least one honest drand threshold member (23-org threshold scheme). Full network capture causes unavailability, not randomness compromise.

**Severity: T2** — Acceptable cryptographic posture. Pure availability attack; no integrity compromise.

---

### Probe 10 — Hex Encoding Attacks (Embedded Null, HTML Injection)

**Attack:** Endpoint returns `"signature": "deadbeef\u0000\uffff<script>…"` — non-hex characters in the signature field.

**Code path:** `hexToBytes` (drand.ts:49): iterates `hex.length / 2` iterations. For each pair, calls `parseInt(hex.slice(i, i+2), 16)`. If the slice contains a non-hex character like `\x00` or `<s`, `parseInt` returns `NaN`. `new Uint8Array` initialized with `NaN` writes `0` for that byte (typed array coerces NaN to 0). The function does not throw — it silently returns a byte array with zeroed-out invalid positions.

**On-chain result:** The mangled signature is almost certainly not a valid G1 point. On-chain `alt_bn128_g1_decompress` returns error `InvalidG1Point` (6001). No funds stolen, but no error surfaced at the SDK fetch layer — the silent mangling is only caught on-chain.

**Prototype pollution:** `hexToBytes` uses `parseInt` on string slices with no `Object` key access, no `JSON.parse` (that already ran earlier), and no property iteration. No prototype pollution vector identified.

**Severity: T3** — On-chain guard catches it. The silent zero-fill in `hexToBytes` is a minor correctness gap worth a future input validation guard.

---

### Probe 11 — Round-Decrementation Loop Abuse

**Attack:** Endpoint consistently returns 404 for any round the SDK requests. On 404 with `round === undefined` (latest-round mode), drand.ts:85-88 decrements `targetRound` and restarts the endpoint inner loop. Attacker returns 404 to every decrement indefinitely.

**Code path:** The decrement only fires when `round === undefined` (auto-round mode) and `response.status === 404`. The `MAX_RETRIES=3` outer loop bounds total attempts to 3 × 5 = 15 endpoint tries. Each decrement breaks out of the inner `for...of DRAND_ENDPOINTS` and starts a new outer iteration — so the total decrement depth is bounded by `MAX_RETRIES` (3 decrements maximum across 3 outer iterations). After that, `AleaError(0, ...)` is thrown.

**Could the attacker serve stale rounds?** If the attacker returns 404 for the current round but a valid beacon for `currentRound - 1`, the SDK accepts it (404 triggers decrement, next attempt for the decremented round returns 200 with that beacon). Combined with Probe 2 (no round cross-check), the attacker can force the SDK to use a beacon that is one round older than current (~3 s) — not exploitable in practice unless the consumer's recency window is extremely tight.

**Severity: T3** — Bounded by retry count. Decrement depth of 3 is not practically exploitable for randomness bias; at worst, the served beacon is 9 s old.

---

## Tiered Findings Summary

### T1 — Critical (silent cryptographic bypass possible)

| ID | Finding | Location |
|----|---------|----------|
| F-01 | `fetchBeacon` accepts `data.round` verbatim without validating it equals the requested `targetRound`. A compromised endpoint can serve a valid older beacon (passing BLS check) for a different round than requested. Consumer receives verified-but-not-current randomness, exploitable if recency window > 3 s. | drand.ts:79-83 |

### T2 — High (user-visible harm, no cryptographic bypass)

| ID | Finding | Location |
|----|---------|----------|
| F-02 | `response.json()` buffers the entire response body with no size cap. A compromised endpoint can OOM the host process by streaming a large body within the 5 s timeout window. | drand.ts:74 |
| F-03 | `fetch` follows HTTP redirects without restriction. A compromised CDN (e.g. Cloudflare edge for `drand.cloudflare.com`) can redirect to an attacker-controlled HTTPS server. | drand.ts:70 |
| F-04 | No client-side chain validation. Wrong-chain beacons cost the user a tx fee + 900K CU before `AleaError(6000)` surfaces. SDK error message does not distinguish wrong-chain from corrupt-sig. | drand.ts:79, client.ts:121 |

### T3 — Low (graceful degradation, ergonomics)

| ID | Finding | Location |
|----|---------|----------|
| F-05 | Slow-loris: 77 s worst-case hang with no caller-controllable global deadline. | drand.ts:58-100 |
| F-06 | Malformed/truncated JSON silently cycles to next endpoint; final error code is `0` (ambiguous). | drand.ts:90 |
| F-07 | `hexToBytes` silently zero-fills invalid hex chars rather than throwing; only caught on-chain. | drand.ts:49-55 |
| F-08 | BGP/DNS takeover of one endpoint: silent fallback, no caller notification. Intentional by design. | constants.ts:10-16 |
| F-09 | Round-decrementation bounded to 3 by retry count; not practically exploitable. | drand.ts:85-88 |

---

## Recommended Fixes (F-01 and F-02 are highest priority)

**F-01 fix:** After `response.ok`, add `if (BigInt(data.round) !== targetRound) { continue; }` before constructing the `DrandBeacon`. This enforces that the returned round matches the one in the URL path.

**F-02 fix:** Pipe through a stream byte-counter or check `Content-Length` header; reject responses over 4 KB (a drand beacon is ~200 bytes).

**F-03 fix:** Add `redirect: "error"` to the `fetch` options object.

**F-04 fix:** Surface chain context in the `AleaError` message for 6000 failures where the signature was fetched (not provided by caller). Or add a client-side note in SDK docs that wrong-chain errors manifest as 6000.
