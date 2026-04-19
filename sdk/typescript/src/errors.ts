export class AleaError extends Error {
  readonly code: number;
  constructor(code: number, message: string) {
    super(message);
    this.name = "AleaError";
    this.code = code;
  }
}

// Frozen at module load so a misbehaving consumer dep can't mutate error
// messages in our address space. Type-level Readonly prevents compile-time
// mutation; Object.freeze() prevents runtime mutation.
//
// Canonical source: programs/alea-verifier/src/errors.rs (ADR 0028
// append-only). This map MUST match the enum 1:1 for every release;
// a CI check (Phase 6) enforces coherence.
//
// Off-chain-only codes (6100+) are for SDK-generated errors that never
// appear on-chain — they signal client/SDK-side failures.
export const ERRORS: Readonly<Record<number, string>> = Object.freeze({
  2001: "ConstraintHasOne: Signer is not the config authority (Anchor auto-generated)",
  3010: "AccountNotSigner: authority account passed without a signature (Anchor auto-generated)",
  6000: "InvalidSignature: BLS signature verification failed",
  6001: "InvalidG1Point: Signature bytes are not a valid G1 point (y² != x³ + 3 mod p)",
  6002: "RoundZero: Round number must be greater than 0",
  6003: "InvalidFieldElement: Reserved (unreachable in v1) — do not retry",
  6004: "NoSquareRoot: hash_round_to_g1 exhausted SVDW candidates (constant corruption or syscall oracle regression; not retryable)",
  6005: "InvalidG2Point: Reserved (unreachable under ADR 0027 fallback path) — do not retry",
  6006: "PairingError: alt_bn128_pairing syscall failed (non-retryable)",
  6007: "WrongChainHash: Config.chain_hash does not match EXPECTED_EVMNET_CHAIN_HASH (wrong-chain deployment)",
  6008: "WrongPubkey: Config.pubkey_g2 does not match EXPECTED_EVMNET_G2_PUBKEY (ADR 0027 fallback) — also emitted by alea-sdk's cpi::verify owner check",
  6009: "ReturnDataMissing: CPI consumer received no return data (Reserved, unreachable under ADR 0030 Pattern A)",
  6010: "InvalidGenesisTime: Config.genesis_time does not match EXPECTED_EVMNET_GENESIS_TIME",
  6011: "InvalidPeriod: Config.period does not match EXPECTED_EVMNET_PERIOD",
  6012: "UnauthorizedInit: initialize authority must equal the program's upgrade_authority_address",
  // --- SDK-side codes (6100+) — never emitted on-chain ---
  6100: "DrandFetchFailed: all drand endpoints failed after retries",
  6101: "DrandRoundMismatch: drand endpoint returned a different round than requested (possibly compromised endpoint)",
  6102: "InvalidInput: SDK-level input validation failed (bad hex, wrong signature length, negative/oversize round, null signer)",
  6103: "Aborted: operation aborted by caller AbortSignal before tx was broadcast",
});
