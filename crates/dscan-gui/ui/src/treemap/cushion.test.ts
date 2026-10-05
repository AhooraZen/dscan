import { describe, expect, test } from "bun:test";
import {
  IA,
  ID,
  LZ,
  computeCushionDerivatives,
  computeCushionNormal,
  computeIllumination,
  shadePixel,
} from "./cushion";
import type { CushionSurface } from "./squarify";

describe("Van Wijk Quadratic Cushion Shading Engine", () => {
  const cushion: CushionSurface = {
    x1: 100,
    x2: 300,
    y1: 100,
    y2: 200,
    depth: 0,
  };

  test("Normal vector at cushion center is exactly (0, 0, 1)", () => {
    const cx = (cushion.x1 + cushion.x2) / 2; // 200
    const cy = (cushion.y1 + cushion.y2) / 2; // 150

    const { nx, ny } = computeCushionDerivatives([cushion], cx, cy);
    expect(Math.abs(nx)).toBeLessThan(1e-6);
    expect(Math.abs(ny)).toBeLessThan(1e-6);

    const normal = computeCushionNormal([cushion], cx, cy);
    expect(Math.abs(normal.x)).toBeLessThan(1e-6);
    expect(Math.abs(normal.y)).toBeLessThan(1e-6);
    expect(Math.abs(normal.z - 1.0)).toBeLessThan(1e-6);
  });

  test("Illumination at center matches Ia + Id * Lz exactly", () => {
    const cx = (cushion.x1 + cushion.x2) / 2;
    const cy = (cushion.y1 + cushion.y2) / 2;

    const normal = computeCushionNormal([cushion], cx, cy);
    const intensity = computeIllumination(normal);

    const expected = IA + ID * LZ;
    expect(Math.abs(intensity - expected)).toBeLessThan(1e-6);
    expect(intensity).toBeGreaterThan(0.8);
    expect(intensity).toBeLessThan(1.0);
  });

  test("test_cushion_normals_boundary: N . [0, 0, 1] > 0 for all boundary points and no NaN/overflow", () => {
    const testPoints = [
      { x: cushion.x1, y: cushion.y1 },
      { x: cushion.x2, y: cushion.y1 },
      { x: cushion.x1, y: cushion.y2 },
      { x: cushion.x2, y: cushion.y2 },
      { x: cushion.x1, y: (cushion.y1 + cushion.y2) / 2 },
      { x: cushion.x2, y: (cushion.y1 + cushion.y2) / 2 },
      { x: (cushion.x1 + cushion.x2) / 2, y: cushion.y1 },
      { x: (cushion.x1 + cushion.x2) / 2, y: cushion.y2 },
    ];

    for (const pt of testPoints) {
      const normal = computeCushionNormal([cushion], pt.x, pt.y);
      expect(Number.isNaN(normal.x)).toBe(false);
      expect(Number.isNaN(normal.y)).toBe(false);
      expect(Number.isNaN(normal.z)).toBe(false);

      // Normal points toward the viewer: Nz > 0
      expect(normal.z).toBeGreaterThan(0);

      // Illumination is non-negative and valid
      const intensity = computeIllumination(normal);
      expect(intensity).toBeGreaterThanOrEqual(IA);
      expect(intensity).toBeLessThanOrEqual(IA + ID);

      // Shaded pixel colors stay within 0..255
      const shaded = shadePixel({ r: 255, g: 128, b: 64 }, intensity);
      expect(shaded.r).toBeGreaterThanOrEqual(0);
      expect(shaded.r).toBeLessThanOrEqual(255);
      expect(shaded.g).toBeGreaterThanOrEqual(0);
      expect(shaded.g).toBeLessThanOrEqual(255);
      expect(shaded.b).toBeGreaterThanOrEqual(0);
      expect(shaded.b).toBeLessThanOrEqual(255);
    }
  });

  test("Nested hierarchical cushions accumulate derivatives smoothly", () => {
    const parent: CushionSurface = { x1: 0, x2: 1000, y1: 0, y2: 1000, depth: 0 };
    const child: CushionSurface = { x1: 100, x2: 300, y1: 100, y2: 200, depth: 1 };

    const normal = computeCushionNormal([parent, child], 150, 150);
    expect(Number.isNaN(normal.x)).toBe(false);
    expect(normal.z).toBeGreaterThan(0);

    const intensity = computeIllumination(normal);
    expect(intensity).toBeGreaterThan(0);
  });
});
