#!/usr/bin/env python3
"""
Seed the verify_beacon fuzz corpus with real drand beacons + adversarial cases.

The verify_beacon fuzz target has:
    #[derive(arbitrary::Arbitrary)]
    struct FuzzInput { round: u64, signature: [u8; 64] }

arbitrary::Arbitrary byte layout for this struct is:
    [round (8 bytes, little-endian)] ++ [signature (64 bytes)]
    = 72 bytes total

Without seeding, random inputs almost never pass the on-curve check
(2^-256 chance), so the fuzzer never exercises pairing code. Seeding with
real signatures (valid on-curve + in-subgroup) means every iteration
reaches pairing; seeding with adversarial inputs exercises every rejection
path we've catalogued.
"""

import json
import struct
import sys
from pathlib import Path

# Resolve repo root relative to this script's location.
SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent.parent.parent
# Fixtures are historically kept in a private directory; the corpus seeds
# below are inlined where possible so this script is runnable without
# external fixture files.
FIXTURES = REPO_ROOT / "build-spec" / "testing" / "fixtures"  # optional; may not exist
CORPUS = REPO_ROOT / "programs" / "alea-verifier" / "fuzz" / "corpus" / "verify_beacon"


def pack_fuzz_input(round_num: int, signature_hex: str) -> bytes:
    sig = bytes.fromhex(signature_hex)
    if len(sig) != 64:
        raise ValueError(f"signature must be 64 bytes, got {len(sig)}")
    if round_num < 0 or round_num > 2**64 - 1:
        raise ValueError(f"round {round_num} out of u64 range")
    blob = struct.pack("<Q", round_num) + sig
    assert len(blob) == 72
    return blob


def seed_valid_beacons(count_out: list) -> None:
    count = 0
    for path in sorted(FIXTURES.glob("round-*.json")):
        with open(path) as f:
            data = json.load(f)
        round_num = data["round"]
        sig_hex = data["signature_hex"]
        blob = pack_fuzz_input(round_num, sig_hex)
        out = CORPUS / f"beacon-valid-{round_num}.bin"
        out.write_bytes(blob)
        print(f"  valid beacon:       round={round_num:>12d} -> {out.name}")
        count += 1
    count_out.append(count)


def seed_adversarial(count_out: list) -> None:
    path = FIXTURES / "adversarial.json"
    with open(path) as f:
        data = json.load(f)
    count = 0
    for case in data["cases"]:
        case_id = case.get("id", "?")
        name = case.get("name", "?")
        if "round" not in case or "signature_hex" not in case:
            print(f"  skipped {case_id} ({name}): missing round or signature_hex", file=sys.stderr)
            continue
        round_num = case["round"]
        sig_hex = case["signature_hex"]
        try:
            blob = pack_fuzz_input(round_num, sig_hex)
        except ValueError as e:
            print(f"  skipped {case_id} ({name}): {e}", file=sys.stderr)
            continue
        out = CORPUS / f"adv-{case_id}-{name}.bin"
        out.write_bytes(blob)
        print(f"  adversarial {case_id}: {name:<35s} -> {out.name}")
        count += 1
    count_out.append(count)


def main() -> int:
    if not FIXTURES.is_dir():
        print(f"ERROR: fixtures dir missing: {FIXTURES}", file=sys.stderr)
        return 1
    CORPUS.mkdir(parents=True, exist_ok=True)

    print(f"Seeding corpus at: {CORPUS}")
    print()

    valid = []
    adv = []
    seed_valid_beacons(valid)
    print()
    seed_adversarial(adv)
    print()

    total = valid[0] + adv[0]
    total_files = len(list(CORPUS.glob("*.bin")))
    print(f"Seeded {valid[0]} valid beacons + {adv[0]} adversarial = {total} new entries")
    print(f"Corpus now contains {total_files} total .bin files")
    if total_files < total:
        print(f"WARNING: expected at least {total} files, found {total_files}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
