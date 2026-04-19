// Hex constants for test fixtures — validated per ADR 0036 (sha256(sig) = randomness)

export const ROUND_1_SIGNATURE_HEX =
  "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346" +
  "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373";

export const ROUND_1_EXPECTED_RANDOMNESS_HEX =
  "781b75698adc3af62cfa55db83cf0c73ae54e1ac8c0d4c3a2224126b65369ec5";

export const ROUND_9337227_SIGNATURE_HEX =
  "01d65d6128f4b2df3d08de85543d8efe06b0281d0770246ae3672e8ddd3efda0" +
  "269373123458f0b5c0073eeed1c816a06809e127421513e34ee07df6987910b3";

export const ROUND_9337227_EXPECTED_RANDOMNESS_HEX =
  "a1e645cd6193837f626716851f5c42ad4bf63ad75193b2cae40f88c08c8f3bd8";

export function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.slice(i, i + 2), 16);
  }
  return bytes;
}

export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}
