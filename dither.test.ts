import { describe, expect, test } from "bun:test";
import { inflateSync } from "node:zlib";
import {
  bayer8Threshold,
  createDitherTexture,
  type DitherVariant,
} from "./dither";

function decode(url: string) {
  expect(url.startsWith("data:image/png;base64,")).toBe(true);
  const png = Buffer.from(url.slice(url.indexOf(",") + 1), "base64");
  expect([...png.subarray(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
  let offset = 8,
    width = 0,
    height = 0;
  const parts: Buffer[] = [];
  while (offset < png.length) {
    const length = png.readUInt32BE(offset),
      type = png.toString("ascii", offset + 4, offset + 8);
    const data = png.subarray(offset + 8, offset + 8 + length);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      expect([...data.subarray(8)]).toEqual([8, 6, 0, 0, 0]);
    }
    if (type === "IDAT") parts.push(data);
    offset += length + 12;
  }
  const raw = inflateSync(Buffer.concat(parts));
  expect(raw.length).toBe((width * 4 + 1) * height);
  return { width, height, raw };
}
describe("ordered dither textures", () => {
  test("Bayer thresholds are periodic and cover each level exactly once", () => {
    const thresholds = Array.from({ length: 64 }, (_, i) =>
      bayer8Threshold(i % 8, Math.floor(i / 8)),
    );
    expect(new Set(thresholds).size).toBe(64);
    expect(thresholds.filter((t) => t < 0.5)).toHaveLength(32);
    expect(bayer8Threshold(-1, -1)).toBe(bayer8Threshold(7, 7));
  });
  test("produces deterministic distinct transparent RGBA PNGs", () => {
    const variants: DitherVariant[] = [
      "ambient",
      "selection",
      "halo",
      "button",
      "meter",
    ];
    const images = variants.map((variant) =>
      createDitherTexture({ width: 64, height: 32, variant, value: 0.7 }),
    );
    expect(new Set(images).size).toBe(5);
    expect(
      createDitherTexture({
        width: 64,
        height: 32,
        variant: "button",
        value: 0.7,
      }),
    ).toBe(images[3]);
    for (const [i, image] of images.entries()) {
      const { width, height, raw } = decode(image);
      expect([width, height]).toEqual([64, 32]);
      const alphas: number[] = [];
      for (let y = 0; y < height; y++)
        for (let x = 0; x < width; x++)
          alphas.push(raw[y * (width * 4 + 1) + 1 + x * 4 + 3]);
      expect(alphas).toContain(0);
      expect(Math.max(...alphas)).toBe([22, 45, 100, 70, 220][i]);
    }
  });
  test("clamps dimensions, strength and meter value", () => {
    const image = createDitherTexture({
      width: -4,
      height: 2.3,
      variant: "meter",
      value: -1,
    });
    const { width, height, raw } = decode(image);
    expect([width, height]).toEqual([1, 2]);
    expect([...raw].every((n) => n === 0)).toBe(true);
    expect(
      createDitherTexture({
        width: 10,
        height: 10,
        variant: "button",
        strength: 5,
      }),
    ).toBe(
      createDitherTexture({
        width: 10,
        height: 10,
        variant: "button",
        strength: 1,
      }),
    );
    expect(
      decode(createDitherTexture({ width: 3000, height: 1, variant: "meter" }))
        .width,
    ).toBe(2048);
  });
  test("meter never draws beyond its value", () => {
    const { raw, width, height } = decode(
      createDitherTexture({
        width: 100,
        height: 12,
        variant: "meter",
        value: 0.35,
      }),
    );
    for (let y = 0; y < height; y++)
      for (let x = 35; x < width; x++)
        expect(raw[y * (width * 4 + 1) + 1 + x * 4 + 3]).toBe(0);
  });
});
