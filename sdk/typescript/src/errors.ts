export class AleaError extends Error {
  code: number;
  constructor(code: number, message: string) {
    super(message);
    this.name = "AleaError";
    this.code = code;
  }
}

export const ERRORS: Record<number, string> = {
  2001: "ConstraintHasOne: Signer is not the config authority (Anchor auto-generated)",
  6000: "InvalidSignature: BLS signature verification failed",
  6001: "InvalidG1Point: Signature bytes are not a valid G1 point",
  6002: "RoundZero: Round number must be greater than 0",
  6003: "InvalidFieldElement: Field element out of valid range [infrastructure]",
  6004: "NoSquareRoot: Square root does not exist [infrastructure]",
  6005: "InvalidG2Point: Public key bytes are not a valid G2 point [infrastructure]",
  6006: "PairingError: alt_bn128_pairing syscall failed [infrastructure]",
  6007: "WrongChainHash: chain_hash does not match EXPECTED_EVMNET_CHAIN_HASH",
  6008: "WrongPubkey: pubkey_g2 does not match EXPECTED_EVMNET_G2_PUBKEY",
  6009: "ReturnDataMissing: CPI consumer received no return data",
};
