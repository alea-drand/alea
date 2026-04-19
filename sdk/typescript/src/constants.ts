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

// T2.O — MAINNET_PROGRAM_ID is a throwing getter until Phase 5 mainnet deploy.
// ESM doesn't allow Object.defineProperty on live bindings, so we export a
// Proxy that throws on any property access, making it safe to import but
// impossible to use without passing { programId } explicitly.
export const MAINNET_PROGRAM_ID: PublicKey = new Proxy({} as PublicKey, {
  get(_target, prop) {
    if (prop === Symbol.toPrimitive || prop === "toString" || prop === "then") {
      return undefined;
    }
    throw new Error(
      "MAINNET_PROGRAM_ID not set. Pass { programId } explicitly until " +
        "@alea/sdk publishes a post-mainnet release with the deployed ID.",
    );
  },
});
