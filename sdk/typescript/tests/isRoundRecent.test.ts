import { describe, it, expect } from "vitest";
import { isRoundRecent } from "../src/drand.js";
import { DRAND_GENESIS_TIME, DRAND_PERIOD } from "../src/constants.js";

const config = {
  genesisTime: BigInt(DRAND_GENESIS_TIME),
  period: BigInt(DRAND_PERIOD),
};

// round 1 timestamp = genesis
// round N timestamp = genesis + (N-1) * period

describe("isRoundRecent", () => {
  it("returns true for a recent round (age = 0)", () => {
    const round = 100n;
    const roundTs = config.genesisTime + (round - 1n) * config.period;
    const clock = { unixTimestamp: roundTs };
    expect(isRoundRecent(round, config, clock, 60n)).toBe(true);
  });

  it("returns true when age == maxAgeSeconds (boundary inclusive)", () => {
    const round = 100n;
    const roundTs = config.genesisTime + (round - 1n) * config.period;
    const clock = { unixTimestamp: roundTs + 60n };
    expect(isRoundRecent(round, config, clock, 60n)).toBe(true);
  });

  it("returns false when age > maxAgeSeconds (stale)", () => {
    const round = 100n;
    const roundTs = config.genesisTime + (round - 1n) * config.period;
    const clock = { unixTimestamp: roundTs + 61n };
    expect(isRoundRecent(round, config, clock, 60n)).toBe(false);
  });

  it("returns false for round = 0", () => {
    const clock = { unixTimestamp: config.genesisTime + 100n };
    expect(isRoundRecent(0n, config, clock, 60n)).toBe(false);
  });

  it("returns false for future round (clock behind round timestamp)", () => {
    const round = 100n;
    const roundTs = config.genesisTime + (round - 1n) * config.period;
    const clock = { unixTimestamp: roundTs - 1n };
    expect(isRoundRecent(round, config, clock, 60n)).toBe(false);
  });

  it("zero-age window: only exact match is recent", () => {
    const round = 50n;
    const roundTs = config.genesisTime + (round - 1n) * config.period;
    expect(isRoundRecent(round, config, { unixTimestamp: roundTs }, 0n)).toBe(true);
    expect(isRoundRecent(round, config, { unixTimestamp: roundTs + 1n }, 0n)).toBe(false);
  });

  it("very large maxAgeSeconds (u64::MAX-like) accepts any past round", () => {
    const round = 1n;
    const roundTs = config.genesisTime;
    // clock far in the future
    const clock = { unixTimestamp: roundTs + 999_999_999n };
    const maxAge = BigInt("18446744073709551615"); // u64::MAX
    expect(isRoundRecent(round, config, clock, maxAge)).toBe(true);
  });

  it("round 1 at genesis with age 0 is recent", () => {
    const clock = { unixTimestamp: config.genesisTime };
    expect(isRoundRecent(1n, config, clock, 30n)).toBe(true);
  });
});
