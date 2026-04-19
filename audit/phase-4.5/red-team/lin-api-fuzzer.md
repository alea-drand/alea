# Lin Wu — API Fuzzer Red Team Report
**Persona:** API fuzzing specialist. Handler of malicious inputs.
**Scope:** `alea-sdk` (Rust) + `@alea/sdk` (TypeScript) — source read-only.
**Method:** For every public function, construct 3–5 hostile inputs. Reason through
the execution path from source. Document expected vs actual. Triage by crash impact.

---

## Probe Log

### Rust — `is_round_recent(round, config, clock, max_age_seconds)`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| R1 | `round=0` | Compute timestamp = `genesis + (0-1)*period` — saturating_sub on u64 → `0*period = 0`, so roundTs = `genesis_time`. Works. Returns false if clock lags genesis. | `round.saturating_sub(1)` → 0 → `genesis_time + 0 = genesis_time`. Arithmetically fine; no panic. Returns a valid bool. | T3 |
| R2 | `round=u64::MAX` | `(u64::MAX - 1) * period` overflows — saturating_mul saturates to `u64::MAX`; `genesis + u64::MAX` saturates to `u64::MAX`; `current - u64::MAX` saturates to 0; `0 <= max_age` → **always returns `true`** | All saturation calls chain correctly per doc comment. BUT: result is `true` — a legitimately bogus future-round round passes the recency check. Any consumer that does `require!(is_round_recent(...))` then hands that round to `cpi::verify` will be stopped on-chain (program rejects round with no beacon). No silent corruption, but the SDK guard is a no-op for u64::MAX. | T2 |
| R3 | `clock.unix_timestamp = i64::MIN` (cast to u64 → huge positive) | `i64::MIN as u64` wraps to `9223372036854775808`. `current_timestamp.saturating_sub(roundTs)` may be a massive number, likely > any realistic `max_age` → **returns false** (rejects). | Cast is on line 162: `clock.unix_timestamp as u64`. No guard. A negative clock (conceivable on new validator with bad NTP) wraps to a very large u64. For realistic `round` values the round timestamp is much smaller, so `huge_u64 - small_roundTs` = huge number >> max_age → rejects the beacon. Outcome is safe (conservative rejection), but the behavior is unspecified and not tested. | T2 |
| R4 | `config.period=0` | `saturating_mul(0) = 0` → `roundTs = genesis_time` for all rounds. Every round maps to genesis. | Correct — no divide-by-zero (multiplication, not division). All rounds look "genesis-old". For large clocks this means `age = clock - genesis` which could exceed any `max_age`, so every beacon is rejected. Surprising but safe. | T3 |
| R5 | `genesis_time > current_timestamp` (future genesis) | `roundTs > current` → `current.saturating_sub(roundTs) = 0 ≤ max_age` → **returns true**. | Consistent with TS `isRoundRecent` design intent (T1.19 comment). Safe: the on-chain verify will fail if the round doesn't exist. No silent data corruption. Not documented. | T3 |
| R6 | `max_age_seconds=0` | Returns true only if `current == roundTs` exactly. Otherwise false. | Correct strict check. Reasonable. | T3 |
| R7 | `max_age_seconds=u64::MAX` | `age <= u64::MAX` always true → always accepts. | All ages pass. Correctly mirrors "no max age" intent. Not panicking. | T3 |

---

### Rust — `config_pda(program_id)`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| C1 | `program_id = Pubkey::default()` (all zeros) | `find_program_address` runs normally — all-zero is a valid Pubkey for PDA derivation purposes. Returns a PDA nobody controls on-chain. | No panic. Returns deterministic (PDA, bump). Consumer would then pass the wrong PDA to their accounts struct — Anchor rejects with constraint error, not a silent corruption. | T3 |
| C2 | `program_id = SystemProgram::id()` | Same — derives a PDA seeded from System Program. On-chain, this PDA will not match the real Alea config account. Anchor constraint check catches it. | No panic. | T3 |
| C3 | Any 32-byte valid Pubkey | Always deterministic — `find_program_address` is pure. | No issues. | — |

---

### Rust — `cpi::verify(...)`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| V1 | `signature = [0u8; 64]` | On-chain program checks G1 validity → `InvalidG1Point` (6001) or `InvalidSignature` (6000). | SDK passes it through — zero bytes are not screened client-side. Correct design for a thin CPI wrapper; validation belongs on-chain. No panic. | T3 |
| V2 | `signature = [0xFF; 64]` | Same — invalid G1 point → on-chain error. | No client panic. Passes through. | T3 |
| V3 | `round=0` | On-chain: `RoundZero` error (6002) per `ERRORS` map. | SDK does not guard round=0 before CPI. Will propagate to on-chain, which returns 6002. No panic. Correct. | T3 |
| V4 | `round=u64::MAX` | No client panic. On-chain behavior undefined (no beacon exists for that round) — network error or 404 from drand, then `InvalidSignature` on-chain. | Passes through cleanly. | T3 |
| V5 | `config` AccountInfo with 0 bytes data | Anchor deserializes Config from AccountInfo on the receiving end. With 0-byte data, `Account<Config>` deserialization panics inside the Alea program. The CPI call returns an error — the consumer receives a `ProgramError`. No panic in the SDK wrapper itself; panic is inside the callee program. | SDK wrapper: `Result::Err` propagated to consumer. No double-panic risk. | T2 |
| V6 | `alea_program = wrong_program_id` | CPI dispatch goes to wrong program. Either that program returns garbage or the runtime refuses the CPI (wrong program for the PDA). Consumer gets `Error`. | No SDK panic. Error propagates. | T3 |

---

### Rust — `PROGRAM_ID`

| # | Probe | Actual | Sev |
|---|-------|--------|-----|
| P1 | `PROGRAM_ID` at runtime | `pub const PROGRAM_ID: Pubkey = alea_verifier::ID` — this is a `const`, not a `static`. `alea_verifier::ID` is generated by `declare_id!` which produces a `const Pubkey`. Const evaluation happens at compile time; any mismatch between the declared vanity ID and the linker-resolved constant is a compile error, not a runtime error. Proven constant by the unit test `program_id_matches_expected_vanity`. | — |

---

### TypeScript — `getCurrentRound()`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| T1 | Normal call | Returns positive bigint. | `BigInt(Math.floor(Date.now() / 1000))` — `Date.now()` cannot return negative in any JS engine (spec: milliseconds since epoch, clamped to 0 if clock is broken). If `now < genesis` (system clock before Oct 2024), result is a **negative bigint** due to `(now - genesis)` going negative before the bigint division. BigInt division in JS truncates toward zero for negative results; then `+1n` may still be negative or 0. | T1 |
| T2 | System clock at 0 (epoch 1970) | `(0 - 1727521075) / 3 + 1` = large negative bigint ≈ `-575840358n`. | Returns a negative round. This round is then passed to `fetchBeacon` which calls `.toString()` on it → fetch URL becomes `.../public/-575840358` → 404 from drand → AleaError after retries. No crash but a confusing error with no validation message. | T2 |

---

### TypeScript — `getRoundAt(timestamp: bigint)`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| G1 | `timestamp = 0n` | Returns `(0 - genesis) / period + 1n` = large negative bigint. | No guard. Returns `-575840358n`. Passed to any downstream consumer (e.g., `fetchBeacon`) without validation. URL becomes `.../public/-575840358`. | T2 |
| G2 | `timestamp = -1n` | Returns even more negative bigint. | No guard against negative timestamps. Same failure path. | T2 |
| G3 | `timestamp` before genesis (any value < 1727521075n) | All produce negative rounds. | No range check. Silent negative bigint return. | T2 |

---

### TypeScript — `isRoundRecent(round, config, clock, maxAgeSeconds)`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| I1 | `round = 0n` | `return false` | Explicit guard: `if (round === 0n) return false`. Clean. | — |
| I2 | `round = -1n` | No guard for negative bigints. `(-1n - 1n) * period = -2n * period` → negative `roundTs`. `clock.unixTimestamp > roundTs` → true → `age = clock - roundTs` = large positive number >> maxAge → **returns false**. | Correct outcome (rejects) but wrong path — arithmetic on negative rounds is not guarded. If `maxAgeSeconds` is also negative (see I4), the comparison `age <= negative` would be false always, which is still safe. | T2 |
| I3 | `config.period = 0n` | `roundTs = genesisTime + (round - 1n) * 0n = genesisTime`. Every round maps to genesis. | Same as Rust — no division, so no panic. All rounds pass/fail based on clock vs genesis + maxAge. | T3 |
| I4 | `maxAgeSeconds = -1n` | `age <= -1n` — age is always non-negative (due to ternary), so this always returns **false** (rejects everything). | No guard on negative maxAge. Silently rejects all rounds. A consumer passing `-1n` gets a function that always says "stale." Confusing but safe. | T2 |
| I5 | `maxAgeSeconds = 0n` | Only passes if age === 0. | Correct strict check. | — |

---

### TypeScript — `fetchBeacon(round?)`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| F1 | `round = 0n` | Round 0 → URL `.../public/0` → drand returns 400 or 404 → AleaError after exhausting retries. | No guard before fetch. Negative round numbers and 0 go through to network. Not a crash but leaks intent to external endpoint. | T2 |
| F2 | `round = undefined` | Uses `getCurrentRound()` which can return negative if system clock is pre-genesis. | Same negative-round cascade as T1/T2 above. | T2 |
| F3 | `round = 18446744073709551616n` (u64::MAX + 1) | URL becomes `.../public/18446744073709551616`. Drand returns 404. AleaError after retries. | No overflow — JS bigint is arbitrary precision. No panic, clean failure. | T3 |

---

### TypeScript — `verifyDrandBeacon({...})`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| VD1 | `signature = new Uint8Array(0)` | `Array.from([])` → empty array passed to Anchor BN serialization as the sig argument → Anchor IDL serializer for `[u8; 64]` receives a 0-length array. Anchor's `borsh` serializer will serialize it as 0 bytes (no length prefix for fixed arrays), producing a malformed instruction buffer. On-chain: instruction data is wrong → `InvalidSignature` or raw borsh panic in the on-chain deserializer. No client crash. | No client validation of signature length before building tx. Borsh will serialize whatever is given — wrong data silently reaches chain. | T1 |
| VD2 | `signature = new Uint8Array(1_000_000)` | `Array.from(new Uint8Array(1_000_000))` = 1M-element JS array passed to Anchor. Borsh serializes 1M bytes. Transaction will exceed Solana's 1232-byte MTU. `sendRawTransaction` returns an error (packet too large). | No client validation of signature length. The 1M-byte tx is built and `serialize()` is called — this will throw or produce an absurd buffer that `sendRawTransaction` rejects. No panic, but the error message will be a low-level serialize error, not an SDK-level "signature must be 64 bytes". | T1 |
| VD3 | `signature = new Uint8Array(32)` (wrong size) | Same as VD1 — undersized array → wrong on-chain instruction data. | No guard. | T1 |
| VD4 | `round = 0n` | `new anchor.BN("0")` → valid BN, serialized as u64 LE = `[0,0,0,0,0,0,0,0]`. On-chain: `RoundZero` (6002). | Correctly propagates to on-chain error. No client crash. | T3 |
| VD5 | `round = 18446744073709551615n` (u64::MAX) | `new anchor.BN(u64::MAX.toString())` — BN handles this correctly. Serialized as `[0xFF;8]`. On-chain: no beacon exists, `InvalidSignature` expected. | No client crash. | T3 |
| VD6 | `signer = null as any` | `isBrowserWallet(null)` → `"sendTransaction" in null` → **TypeError: Cannot use 'in' operator to search for 'sendTransaction' in null**. Unhandled rejection. | `in` operator on null throws at runtime. No guard. | T1 |

---

### TypeScript — `createVerifyInstruction({round, signature, payer, programId?})`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| CI1 | `round = -1n` | `roundBuf.writeBigUInt64LE(-1n)` → Node `Buffer.writeBigUInt64LE` **throws `RangeError: The value of "value" is out of range`** for negative bigints. | No guard on round before `writeBigUInt64LE`. Direct unhandled throw. | T1 |
| CI2 | `round = 18446744073709551616n` (u64::MAX + 1) | `writeBigUInt64LE` throws `RangeError` — value exceeds u64 range. | No guard. Throws at serialization layer. Error message is a Node internals RangeError, not an SDK validation error. | T1 |
| CI3 | `signature = new Uint8Array(0)` | `Buffer.from([])` → 0-byte sigBuf. `Buffer.concat([disc, roundBuf, sigBuf])` = 16-byte data (discriminator + round). On-chain borsh deserializer expects 72 more bytes → raw decode error. | No client validation. Wrong-length instruction silently sent. | T1 |
| CI4 | `signature = new Uint8Array(1_000_000)` | Builds a >1MB instruction data buffer. `new TransactionInstruction` accepts any data buffer. Caller would get a 1MB+ instruction object. Likely fails at signing or send time with a confusing low-level error. | No size guard. | T2 |
| CI5 | `payer = new PublicKey(new Uint8Array(32))` (all-zeros) | Instruction is built with zero pubkey as payer signer. TX would fail to sign if the keypair doesn't match. | Structurally valid. Accepted by `createVerifyInstruction`. On-chain: signature mismatch error. | T3 |

---

### TypeScript — `getConfigAddress(programId?)`

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| GA1 | `programId = new PublicKey(new Uint8Array(32))` (all-zeros) | `findProgramAddressSync` with all-zero programId. Solana's PDA derivation accepts any 32-byte key. Returns a PDA for the zero program — which cannot own anything on-chain. | No panic. Returns wrong PDA. Consumer will get account-not-found or constraint error on-chain. | T3 |
| GA2 | `programId = SystemProgram.programId` | Same path — derives PDA under System Program. Returns wrong PDA. | No panic. | T3 |
| GA3 | `programId = MAINNET_PROGRAM_ID` | Proxy access to `.toString()` / `.toBuffer()` inside `findProgramAddressSync` → Proxy get trap fires on any property → **throws the MAINNET not-set Error**. | This is intended behavior for Phase 4, but the error message fires inside `findProgramAddressSync` deep in web3.js, making it look like a web3.js internal error rather than an SDK guard. Stack trace is misleading. | T2 |

---

### TypeScript — `hexToBytes(hex)` (internal)

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| H1 | Odd-length hex `"abc"` | `hex.length / 2 = 1.5` → `new Uint8Array(1.5)` → `Uint8Array(1)` (truncated to int). Loop runs `i = 0, 2` — at `i=2`, `hex.slice(2, 4) = "c"` → `parseInt("c", 16) = 12`, stored at `bytes[1]` → **out-of-bounds write** into a 1-element Uint8Array. JS ignores OOB writes on typed arrays (they're no-ops). Result: `Uint8Array([0xab])` — last nibble `c` is silently dropped. | Silent data truncation. Not a crash. A caller passing an odd-length signature hex gets a shorter-than-expected byte array, which then passes to on-chain verify as a wrong-length signature. No error is raised. | T1 |
| H2 | Non-hex chars `"zz"` | `parseInt("zz", 16) = NaN` → `NaN | 0 = 0` (implicit coercion in typed array assignment). Silently produces zero bytes. | Silent bad data. No error. | T1 |
| H3 | Empty string `""` | `new Uint8Array(0)` → loop doesn't run → returns `Uint8Array([])`. | No crash. Returns empty. Downstream receives wrong-size signature. | T2 |
| H4 | Unicode `"🔥ff"` | `"🔥ff".length = 5` (emoji is 2 code units) → `new Uint8Array(2.5)` → `Uint8Array(2)`. Loop slices incorrectly across emoji boundary → `parseInt("🔥", 16) = NaN → 0`, `parseInt("ff", 16) = 255`. Result: `Uint8Array([0, 255])`. | Silent wrong output. No exception. | T1 |

---

### TypeScript — `extractErrorCode(err)` (internal)

| # | Input | Expected | Actual | Sev |
|---|-------|----------|--------|-----|
| E1 | `logs` array containing `"Error Number: 99999"` | Returns 99999 — a code not in `ERRORS` map. | `ERRORS[99999]` → `undefined` → caller throws `AleaError(99999, "Unknown error code 99999")`. Valid path. | T3 |
| E2 | Attacker-controlled log line `"Program log: Error Number: 6000"` embedded in a non-error response | If `info.meta?.err` is null (success) this code path isn't reached. Only parsed when `info.meta?.err` is truthy. | Not reachable via injection on a successful tx. Safe. | — |
| E3 | `logs` array with 10,000 entries each 10KB | Iterates all entries calling `.match()`. No limit. Could be slow if a malicious RPC feeds huge log arrays. | No DoS guard. Unbounded log scan. | T2 |

---

### TypeScript — `MAINNET_PROGRAM_ID` Proxy Exhaustive Check

| Access | Throws? |
|--------|---------|
| `.toBase58()` | Yes — `get` trap fires, throws |
| `.toString()` | No — proxy returns `undefined` for `toString` (explicit carve-out), so `undefined()` → TypeError, not the intended AleaError |
| `.then` | No — returns `undefined` (explicit carve-out). Good for `await` safety. |
| `Symbol.toPrimitive` | No — returns `undefined` (explicit carve-out). |
| `.equals(x)` | Yes — `get` fires, throws AleaError |
| `JSON.stringify(MAINNET_PROGRAM_ID)` | No crash — JSON.stringify calls `toJSON` first (not carved out → throws AleaError on `toJSON` property access). So JSON.stringify throws AleaError. |
| `"toBase58" in MAINNET_PROGRAM_ID` | No — `has` trap is not defined on the proxy. `in` operator uses the default `has` which returns false on the empty `{}` target. **Does NOT throw.** |
| `for...in MAINNET_PROGRAM_ID` | Does not throw — no `ownKeys` trap. Iterates nothing. |
| Spread `{...MAINNET_PROGRAM_ID}` | Calls `ownKeys` trap (not defined) → default → returns `[]`. No throw. Result is `{}`. |
| `Symbol.iterator` | `get` trap fires → throws AleaError (not in carve-out). |

**Finding:** `toString()` access returns `undefined` rather than throwing. A consumer calling `MAINNET_PROGRAM_ID.toString()` gets `undefined`, not an error — they can silently pass `undefined` into a string context (e.g., URL building) without any indication they've used the unset constant. The carve-out for `toString` is overly broad.

---

## Tiered Findings

### T1 — Crashes / Panics / Silent Corruption

| ID | Location | Finding |
|----|----------|---------|
| **T1-A** | `instruction.ts: createVerifyInstruction` | `writeBigUInt64LE` throws unguarded `RangeError` for any negative `round` bigint or any value > u64::MAX. Error is a Node internal, not an SDK validation message. Inputs: `round = -1n`, `round = u64::MAX + 1n`. |
| **T1-B** | `drand.ts: hexToBytes` | Odd-length hex strings cause silent last-nibble truncation (OOB write is a no-op in JS typed arrays). Non-hex chars (`"zz"`) and unicode produce silent zero bytes. No error raised. Any caller using this output as a signature gets wrong data sent to chain. Inputs: `"abc"`, `"zz"`, `"🔥ff"`. |
| **T1-C** | `client.ts: verifyDrandBeacon` | `isBrowserWallet(null)` → `"sendTransaction" in null` → uncaught TypeError. No null guard on `signer`. Unhandled rejection surfaces to caller. Input: `signer = null as any`. |
| **T1-D** | `client.ts: verifyDrandBeacon` | Signature length is not validated before `Array.from(signature)` is passed to Anchor. A 0-byte, 32-byte, or 1M-byte signature silently builds a malformed transaction. Inputs: `new Uint8Array(0)`, `new Uint8Array(32)`, `new Uint8Array(1_000_000)`. |

### T2 — Confusing Error Surface

| ID | Location | Finding |
|----|----------|---------|
| **T2-A** | `drand.ts: getCurrentRound / getRoundAt` | Pre-genesis timestamps (system clock before Oct 2024, or any negative/zero input) produce negative bigint rounds with no validation. Downstream failure is a drand 404 after all retries, not a descriptive SDK error. |
| **T2-B** | `drand.ts: isRoundRecent` | Negative `round` (e.g., `-1n`) and negative `maxAgeSeconds` are not guarded. Behavior is safe (rejects) but the arithmetic pathway over negative bigints is unintentional and undocumented. |
| **T2-C** | `constants.ts: MAINNET_PROGRAM_ID` | `MAINNET_PROGRAM_ID.toString()` returns `undefined` (explicit carve-out) instead of throwing. A consumer concatenating the result into a URL or log gets `"undefined"` with no error signal. |
| **T2-D** | `lib.rs: is_round_recent` | `round = u64::MAX` saturates to `current_timestamp = 0 <= max_age` → always returns `true`. The SDK recency guard is bypassed by an impossibly large round number. On-chain verify will still reject (no beacon), but the SDK's own guard provides false assurance. |
| **T2-E** | `lib.rs: is_round_recent` | Negative `clock.unix_timestamp` (i64 → u64 cast, wraps to large u64) is not documented. Behavior is conservative (rejects), but the cast semantics are a footgun for consumers on non-standard validators. |

### T3 — Works But Undocumented Edge Cases

| ID | Location | Finding |
|----|----------|---------|
| **T3-A** | `is_round_recent` with `period=0` | All rounds map to genesis; none pass if clock > genesis + max_age. Correct but unspecified behavior. |
| **T3-B** | `is_round_recent` with future genesis | All rounds pass the check. Documented in TS but not in Rust. |
| **T3-C** | `createVerifyInstruction` with `signature = new Uint8Array(1_000_000)` | Builds valid JS instruction object; fails only at serialization/send layer with a low-level error. |
| **T3-D** | `fetchBeacon` with `round = u64::MAX + 1n` | Arbitrary-precision bigint — no client overflow. Clean 404 failure from drand. |

---

## Summary

**Top 3 concrete vulnerabilities:**

1. **`hexToBytes` silent corruption (T1-B):** Odd-length or non-hex input silently truncates/zeros bytes with no error. A consumer passing a malformed hex signature string from an untrusted source gets wrong bytes sent to the on-chain verifier — no exception, no warning. Fix: assert `hex.length % 2 === 0` and `/^[0-9a-fA-F]*$/.test(hex)` before processing.

2. **`createVerifyInstruction` RangeError on negative round (T1-A):** `writeBigUInt64LE(-1n)` throws an undescriptive Node `RangeError`. Any consumer calling this function with user-supplied round values without pre-validating sign gets an uncaught internal error. Fix: guard `if (options.round < 0n || options.round > 18446744073709551615n) throw new AleaError(...)` before the buffer write.

3. **`verifyDrandBeacon` no signature length check (T1-D):** The signature byte array is passed directly to Anchor's borsh serializer as a fixed-size `[u8; 64]` field. Passing 0, 32, or 1M bytes produces a malformed instruction with no client-side error. Fix: `if (args.signature.length !== 64) throw new AleaError(6001, "signature must be exactly 64 bytes")`.
