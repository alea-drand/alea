// scripts/attest-evmnet-pubkey.go — off-chain attestation that
// EXPECTED_EVMNET_PUBKEY is in BN254 G2's prime-order subgroup.
//
// T1.07 (Phase 2.5 Wave E) — the ADR 0027 fallback path chose the
// hardcoded-pubkey strategy because on-chain subgroup check exceeds
// 1.4M CU on BPF. That made the constant the SOLE cryptographic trust
// anchor: if drand ever ships a non-subgroup key (steward error, CI
// misconfiguration, compromise), Alea accepts it silently and pairing
// is soundness-broken for signatures under that key.
//
// This script runs gnark-crypto's `IsOnCurve()` + `IsInSubGroup()` on
// the hardcoded 128 bytes at `programs/alea-verifier/src/crypto/
// constants.rs:57-74` and emits an attestation JSON with the commit
// hash, gnark version, and validation result.
//
// CI usage (intended):
//   Run on every PR that modifies constants.rs or this script.
//   Writes an attestation JSON file with the timestamp + expected hex
//   for external-auditor reference.
//
// Run:
//   cd scripts
//   go mod init attest-evmnet-pubkey 2>/dev/null || true
//   go get github.com/consensys/gnark-crypto/ecc/bn254
//   go run attest-evmnet-pubkey.go
//
// Output format (success):
//   {"is_on_curve":true,"is_in_subgroup":true,"pubkey_sha256":"<64-hex>",
//    "verified_at_commit":"<sha>","gnark_crypto_version":"<ver>",
//    "timestamp_utc":"<iso8601>","result":"PASS"}
//
// Exit codes:
//   0 = both IsOnCurve + IsInSubGroup passed
//   1 = IsOnCurve failed (constant corrupted)
//   2 = IsInSubGroup failed (security-critical; ADR 0027 assumption broken)
//   3 = unexpected error

package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math/big"
	"os"
	"os/exec"
	"strings"
	"time"

	bn254 "github.com/consensys/gnark-crypto/ecc/bn254"
	"github.com/consensys/gnark-crypto/ecc/bn254/fp"
)

// Hardcoded constant from programs/alea-verifier/src/crypto/constants.rs:57-74
// (EIP-197 encoding: x_c1 || x_c0 || y_c1 || y_c0, big-endian 32-byte limbs).
// MUST stay byte-for-byte identical with the Rust constant.
var evmnetPubkeyBytes = []byte{
	0x07, 0xe1, 0xd1, 0xd3, 0x35, 0xdf, 0x83, 0xfa,
	0x98, 0x46, 0x20, 0x05, 0x69, 0x03, 0x72, 0xc6,
	0x43, 0x34, 0x00, 0x60, 0xd2, 0x05, 0x30, 0x6a,
	0x9a, 0xa8, 0x10, 0x6b, 0x6b, 0xd0, 0xb3, 0x82,
	0x05, 0x57, 0xec, 0x32, 0xc2, 0xad, 0x48, 0x8e,
	0x4d, 0x4f, 0x60, 0x08, 0xf8, 0x9a, 0x34, 0x6f,
	0x18, 0x49, 0x20, 0x92, 0xcc, 0xc0, 0xd5, 0x94,
	0x61, 0x0d, 0xe2, 0x73, 0x2c, 0x8b, 0x80, 0x8f,
	0x00, 0x95, 0x68, 0x5a, 0xe3, 0xa8, 0x5b, 0xa2,
	0x43, 0x74, 0x7b, 0x1b, 0x2f, 0x42, 0x60, 0x49,
	0x01, 0x0f, 0x6b, 0x73, 0xa0, 0xcf, 0x1d, 0x38,
	0x93, 0x51, 0xd5, 0xaa, 0xaa, 0x10, 0x47, 0xf6,
	0x29, 0x7d, 0x3a, 0x4f, 0x97, 0x49, 0xb3, 0x3e,
	0xb2, 0xd9, 0x04, 0xc9, 0xd9, 0xeb, 0xf1, 0x72,
	0x24, 0x15, 0x0d, 0xdd, 0x7a, 0xbd, 0x75, 0x67,
	0xa9, 0xbe, 0xc6, 0xc7, 0x44, 0x80, 0xee, 0x0b,
}

type Attestation struct {
	IsOnCurve          bool   `json:"is_on_curve"`
	IsInSubGroup       bool   `json:"is_in_subgroup"`
	PubkeySHA256       string `json:"pubkey_sha256"`
	VerifiedAtCommit   string `json:"verified_at_commit"`
	GnarkCryptoVersion string `json:"gnark_crypto_version"`
	TimestampUTC       string `json:"timestamp_utc"`
	Result             string `json:"result"`
	PubkeyHex          string `json:"pubkey_hex"`
}

func gitCommitSHA() string {
	out, err := exec.Command("git", "rev-parse", "HEAD").Output()
	if err != nil {
		return "unknown"
	}
	return strings.TrimSpace(string(out))
}

func gnarkVersion() string {
	// Best-effort. Parse go.mod line if present.
	out, err := exec.Command("go", "list", "-m", "github.com/consensys/gnark-crypto").Output()
	if err != nil {
		return "unknown"
	}
	return strings.TrimSpace(string(out))
}

func main() {
	if len(evmnetPubkeyBytes) != 128 {
		fmt.Fprintf(os.Stderr, "ERROR: pubkey bytes must be 128, got %d\n", len(evmnetPubkeyBytes))
		os.Exit(3)
	}

	// EIP-197 G2 encoding: x_c1 || x_c0 || y_c1 || y_c0, each 32 BE bytes.
	// gnark-crypto's G2Affine uses (x, y) where x = c0 + c1*u, y = c0 + c1*u.
	// We need to build fp.Elements from the BE bytes.
	var point bn254.G2Affine
	var err error

	point.X.A1, err = fpFromBE(evmnetPubkeyBytes[0:32])
	if err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: x_c1 decode failed: %v\n", err)
		os.Exit(3)
	}
	point.X.A0, err = fpFromBE(evmnetPubkeyBytes[32:64])
	if err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: x_c0 decode failed: %v\n", err)
		os.Exit(3)
	}
	point.Y.A1, err = fpFromBE(evmnetPubkeyBytes[64:96])
	if err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: y_c1 decode failed: %v\n", err)
		os.Exit(3)
	}
	point.Y.A0, err = fpFromBE(evmnetPubkeyBytes[96:128])
	if err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: y_c0 decode failed: %v\n", err)
		os.Exit(3)
	}

	onCurve := point.IsOnCurve()
	inSubGroup := point.IsInSubGroup()

	pubkeyHashRaw := sha256.Sum256(evmnetPubkeyBytes)
	pubkeyHash := hex.EncodeToString(pubkeyHashRaw[:])

	result := "PASS"
	if !onCurve {
		result = "FAIL_NOT_ON_CURVE"
	} else if !inSubGroup {
		result = "FAIL_NOT_IN_SUBGROUP"
	}

	att := Attestation{
		IsOnCurve:          onCurve,
		IsInSubGroup:       inSubGroup,
		PubkeySHA256:       pubkeyHash,
		VerifiedAtCommit:   gitCommitSHA(),
		GnarkCryptoVersion: gnarkVersion(),
		TimestampUTC:       time.Now().UTC().Format(time.RFC3339),
		Result:             result,
		PubkeyHex:          hex.EncodeToString(evmnetPubkeyBytes),
	}

	j, _ := json.MarshalIndent(att, "", "  ")
	fmt.Println(string(j))

	if !onCurve {
		os.Exit(1)
	}
	if !inSubGroup {
		os.Exit(2)
	}
	os.Exit(0)
}

// fpFromBE builds a bn254 fp.Element from a 32-byte big-endian byte slice.
// Validates x < p; returns error otherwise.
func fpFromBE(be []byte) (fp.Element, error) {
	var e fp.Element
	bi := new(big.Int).SetBytes(be)
	// p is gnark's bn254 base field modulus; IsValid checks x < p.
	e.SetBigInt(bi)
	// Re-check that round-trip matches (catches non-canonical encoding).
	back := make([]byte, 32)
	_ = back
	bytes32 := e.Bytes()
	if hex.EncodeToString(bytes32[:]) != hex.EncodeToString(be) {
		return fp.Element{}, fmt.Errorf("non-canonical fp element: input %x != roundtrip %x", be, bytes32)
	}
	return e, nil
}
