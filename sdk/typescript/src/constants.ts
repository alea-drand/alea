import { PublicKey } from "@solana/web3.js";

export const DRAND_CHAIN_HASH =
  "04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3";

export const DRAND_GENESIS_TIME = 1727521075;

export const DRAND_PERIOD = 3;

export const DRAND_ENDPOINTS: readonly string[] = [
  "https://api.drand.sh",
  "https://api2.drand.sh",
  "https://api3.drand.sh",
  "https://drand.cloudflare.com",
  "https://api.drand.secureweb3.com:6875",
];

export const DEVNET_PROGRAM_ID = new PublicKey(
  "ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U",
);

// Phase 4.5 T2-08: tighten the throw carve-outs. Previous impl silently
// returned undefined for `toString` and `Symbol.toPrimitive` so that
// `console.log(MAINNET_PROGRAM_ID)` wouldn't crash, but that let
// `MAINNET_PROGRAM_ID.toString()` return "undefined" — consumers using
// it in template strings or explorer-URL construction got invalid
// strings silently. Now throws on those too. We keep the `then` carve-
// out so that accidentally awaiting an import doesn't hang forever.
//
// ESM doesn't allow Object.defineProperty on live bindings, so we export
// a Proxy that throws on every property access.
const MAINNET_THROW_MESSAGE =
  "MAINNET_PROGRAM_ID not set (v0.1.x is devnet-only). Pass { programId } " +
  "explicitly, or wait for the post-Phase-5 release that bakes in the " +
  "deployed mainnet ID. This symbol intentionally throws to prevent silent " +
  "wrong-network deployments.";

export const MAINNET_PROGRAM_ID: PublicKey = new Proxy({} as PublicKey, {
  get(_target, prop) {
    // `then` carve-out: if someone accidentally `await`s an import that
    // destructures MAINNET_PROGRAM_ID, Promise resolution probes `.then`
    // and would loop forever. Returning undefined breaks the probe cleanly.
    if (prop === "then") return undefined;
    throw new Error(MAINNET_THROW_MESSAGE);
  },
  has() {
    throw new Error(MAINNET_THROW_MESSAGE);
  },
  ownKeys() {
    throw new Error(MAINNET_THROW_MESSAGE);
  },
  getPrototypeOf() {
    throw new Error(MAINNET_THROW_MESSAGE);
  },
});
