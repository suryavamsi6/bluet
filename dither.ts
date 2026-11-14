import { deflateSync } from "node:zlib";

export type DitherVariant =
  "ambient" | "selection" | "halo" | "button" | "meter";
export type DitherOptions = {
  width: number;
  height: number;
  variant: DitherVariant;
  strength?: number;
  value?: number;
};

// Bayer matrix recursion and threshold adapted from dither-kit-rs by Deepanshu
// Mishra, MIT licensed; see assets/licenses/DitherKit-LICENSE.
const B2 = [0, 2, 3, 1];
const B4 = Array.from({ length: 16 }, (_, i) => {
  const x = i % 4,
    y = Math.floor(i / 4);
  return (
    4 * B2[(y % 2) * 2 + (x % 2)] +
    B2[Math.floor(y / 2) * 2 + Math.floor(x / 2)]
  );
});
const B8 = Array.from({ length: 64 }, (_, i) => {
  const x = i % 8,
    y = Math.floor(i / 8);
  return (
    4 * B4[(y % 4) * 4 + (x % 4)] +
    B2[Math.floor(y / 4) * 2 + Math.floor(x / 4)]
  );
});
export function bayer8Threshold(x: number, y: number): number {
  const mod = (n: number) => ((Math.trunc(n) % 8) + 8) % 8;
  return (B8[mod(y) * 8 + mod(x)] + 0.5) / 64;
}
const clamp = (v: number, low: number, high: number) =>
  Math.min(high, Math.max(low, v));
const dim = (v: number) =>
  clamp(Number.isFinite(v) ? Math.round(v) : 1, 1, 2048);
const unit = (v: number | undefined, fallback: number) =>
  Math.round(clamp(Number.isFinite(v) ? v! : fallback, 0, 1) * 100) / 100;
const cache = new Map<string, string>();
const CRC_TABLE = Uint32Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 255] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(type: string, data: Buffer): Buffer {
  const name = Buffer.from(type, "ascii");
  const out = Buffer.allocUnsafe(data.length + 12);
  out.writeUInt32BE(data.length, 0);
  name.copy(out, 4);
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}
function density(
  variant: DitherVariant,
  x: number,
  y: number,
  value: number,
): number {
  switch (variant) {
    case "ambient": {
      const wave = 0.5 + 0.5 * Math.sin(x * 9 + Math.sin(y * 7) * 1.5 + y * 5);
      const curtain = Math.exp(
        -Math.pow((y - (0.35 + 0.12 * Math.sin(x * 6))) / 0.36, 2),
      );
      return (
        (0.09 + 0.28 * wave * curtain) *
        Math.sin(Math.PI * clamp(y, 0, 1)) *
        Math.sin((Math.PI * clamp(x, 0, 1)) / 2)
      );
    }
    case "selection":
      return 0.45 * Math.pow(x, 1.5) * (0.35 + 0.65 * Math.pow(y, 1.8));
    case "halo": {
      const r = Math.hypot((x - 0.5) * 2, (y - 0.5) * 2);
      return (
        0.55 *
        Math.exp(-Math.pow((r - 0.53) / 0.25, 2)) *
        clamp((1 - r) * 5, 0, 1)
      );
    }
    case "button":
      return 0.24 + 0.18 * (1 - y) + 0.07 * Math.sin(x * Math.PI * 2 + y * 5);
    case "meter": {
      if (value <= 0 || x >= value) return 0;
      return 0.78 * clamp((value - x) * 32, 0, 1);
    }
  }
}
export function createDitherTexture(options: DitherOptions): string {
  const width = dim(options.width),
    height = dim(options.height);
  const strength = unit(options.strength, 1),
    value = unit(options.value, 1);
  const key = `${width}:${height}:${options.variant}:${strength}:${value}`;
  const cached = cache.get(key);
  if (cached) {
    cache.delete(key);
    cache.set(key, cached);
    return cached;
  }
  const cell =
    options.variant === "halo" || options.variant === "meter" ? 1 : 2;
  const alpha = {
    ambient: 22,
    selection: 45,
    halo: 100,
    button: 70,
    meter: 220,
  }[options.variant];
  const color = options.variant === "meter" ? [117, 217, 163] : [91, 146, 245];
  const pixels = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    const row = y * (width * 4 + 1);
    for (let x = 0; x < width; x++) {
      const gx = Math.floor(x / cell),
        gy = Math.floor(y / cell);
      const fx = (gx * cell + cell / 2) / width,
        fy = (gy * cell + cell / 2) / height;
      if (
        density(options.variant, fx, fy, value) * strength <=
        bayer8Threshold(gx, gy)
      )
        continue;
      const i = row + 1 + x * 4;
      pixels[i] = color[0];
      pixels[i + 1] = color[1];
      pixels[i + 2] = color[2];
      pixels[i + 3] = alpha;
    }
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8;
  header[9] = 6;
  const png = Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(pixels)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
  const result = `data:image/png;base64,${png.toString("base64")}`;
  cache.set(key, result);
  if (cache.size > 24) cache.delete(cache.keys().next().value!);
  return result;
}
