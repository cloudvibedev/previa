/**
 * UUIDv7 generator — time-ordered, sortable UUIDs.
 * Format: tttttttt-tttt-7xxx-yxxx-xxxxxxxxxxxx
 *   - t = 48-bit Unix timestamp in milliseconds
 *   - 7 = version
 *   - x = random bits
 *   - y = variant (8, 9, a, b)
 */
export function generateUUID(): string {
  const now = Date.now();

  // 48-bit timestamp
  const msHex = now.toString(16).padStart(12, "0");

  // Random bytes for the rest
  const randBytes = new Uint8Array(10);
  crypto.getRandomValues(randBytes);

  // Build hex parts
  const timeLow = msHex.slice(0, 8);           // 8 hex = 32 bits
  const timeMid = msHex.slice(8, 12);           // 4 hex = 16 bits

  // time_hi_and_version: version 7 (0111) + 12 random bits
  const randHi = ((randBytes[0] & 0x0f) | 0x70).toString(16).padStart(2, "0")
    + randBytes[1].toString(16).padStart(2, "0");

  // clock_seq_hi_and_variant: variant 10 + 6 random bits, then 8 random bits
  const variantByte = ((randBytes[2] & 0x3f) | 0x80).toString(16).padStart(2, "0")
    + randBytes[3].toString(16).padStart(2, "0");

  // node: 48 random bits
  const node = Array.from(randBytes.slice(4, 10))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");

  return `${timeLow}-${timeMid}-${randHi}-${variantByte}-${node}`;
}
