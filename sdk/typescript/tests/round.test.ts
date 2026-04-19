import { describe, it, expect, vi, afterEach } from "vitest";
import { getCurrentRound, getRoundAt } from "../src/drand.js";
import { DRAND_GENESIS_TIME, DRAND_PERIOD } from "../src/constants.js";

const GENESIS = BigInt(DRAND_GENESIS_TIME);
const PERIOD = BigInt(DRAND_PERIOD);

afterEach(() => {
  vi.restoreAllMocks();
});

describe("getCurrentRound", () => {
  it("returns 1 at genesis time", () => {
    vi.spyOn(Date, "now").mockReturnValue(Number(GENESIS) * 1000);
    expect(getCurrentRound()).toBe(1n);
  });

  it("returns 2 at genesis + period", () => {
    vi.spyOn(Date, "now").mockReturnValue(Number(GENESIS + PERIOD) * 1000);
    expect(getCurrentRound()).toBe(2n);
  });

  it("returns 100 at genesis + 99 * period", () => {
    vi.spyOn(Date, "now").mockReturnValue(Number(GENESIS + 99n * PERIOD) * 1000);
    expect(getCurrentRound()).toBe(100n);
  });

  it("returns correct bigint for a known timestamp", () => {
    // Round 9337227 at timestamp = GENESIS + (9337227 - 1) * PERIOD
    const ts = GENESIS + 9337226n * PERIOD;
    vi.spyOn(Date, "now").mockReturnValue(Number(ts) * 1000);
    expect(getCurrentRound()).toBe(9337227n);
  });

  it("returns a bigint (not a number)", () => {
    vi.spyOn(Date, "now").mockReturnValue(Number(GENESIS) * 1000);
    expect(typeof getCurrentRound()).toBe("bigint");
  });
});

describe("getRoundAt", () => {
  it("returns 1 at genesis timestamp", () => {
    expect(getRoundAt(GENESIS)).toBe(1n);
  });

  it("returns 2 at genesis + period", () => {
    expect(getRoundAt(GENESIS + PERIOD)).toBe(2n);
  });

  it("symmetric with getCurrentRound for same timestamp", () => {
    const ts = GENESIS + 12345n * PERIOD;
    vi.spyOn(Date, "now").mockReturnValue(Number(ts) * 1000);
    expect(getRoundAt(ts)).toBe(getCurrentRound());
  });

  it("round 9337227 matches expected timestamp", () => {
    const expectedRound = 9337227n;
    const ts = GENESIS + (expectedRound - 1n) * PERIOD;
    expect(getRoundAt(ts)).toBe(expectedRound);
  });
});
