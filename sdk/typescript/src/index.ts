// @alea/sdk — public API surface per T2.Q

export { getVerifiedRandomness, verifyDrandBeacon } from "./client.js";
export { fetchBeacon, getCurrentRound, getRoundAt, isRoundRecent } from "./drand.js";
export { createVerifyInstruction, getConfigAddress } from "./instruction.js";
export {
  DRAND_CHAIN_HASH,
  DRAND_GENESIS_TIME,
  DRAND_PERIOD,
  DRAND_ENDPOINTS,
  DEVNET_PROGRAM_ID,
  MAINNET_PROGRAM_ID,
} from "./constants.js";
export { AleaError, ERRORS } from "./errors.js";
export type { DrandBeacon } from "./drand.js";
export type { DrandConfig, SolanaClock, BeaconResult, VerifyOptions } from "./types.js";
