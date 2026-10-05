/**
 * Van Wijk Quadratic Cushion Treemap Illumination Model.
 *
 * Implements Jarke J. van Wijk's quadratic surface model with ambient + diffuse shading:
 *   I = I_a + I_d * max(0, N . L)
 */

import { type ColorRgb, hexToRgb } from "../styles/palette";
import type { CushionSurface, TreemapLayoutItem } from "./squarify";

export const H0 = 0.5;
export const F = 0.65;
export const IA = 0.25;
export const ID = 0.75;

// Light vector pointing from top-left: L = (-0.5, -0.5, 1.0) normalized
const L_LEN = Math.sqrt(0.5 * 0.5 + 0.5 * 0.5 + 1.0 * 1.0);
export const LX = -0.5 / L_LEN; // -0.408248290463863
export const LY = -0.5 / L_LEN; // -0.408248290463863
export const LZ = 1.0 / L_LEN;  //  0.816496580927726

export interface Vector3 {
  x: number;
  y: number;
  z: number;
}

/**
 * Computes partial derivatives of quadratic cushion surface at (x, y)
 */
export function computeCushionDerivatives(
  cushions: CushionSurface[],
  x: number,
  y: number
): { nx: number; ny: number } {
  let nx = 0;
  let ny = 0;

  for (let i = 0; i < cushions.length; i++) {
    const c = cushions[i];
    const w = c.x2 - c.x1;
    const h = c.y2 - c.y1;
    if (w <= 0 || h <= 0) continue;

    const hk = H0 * Math.pow(F, c.depth);
    const wSq = w * w;
    const hSq = h * h;

    const dzdx = -4 * hk * (2 * x - (c.x1 + c.x2)) / wSq;
    const dzdy = -4 * hk * (2 * y - (c.y1 + c.y2)) / hSq;

    nx += dzdx;
    ny += dzdy;
  }

  return { nx, ny };
}

/**
 * Computes surface normal N at (x, y)
 */
export function computeCushionNormal(
  cushions: CushionSurface[],
  x: number,
  y: number
): Vector3 {
  const { nx, ny } = computeCushionDerivatives(cushions, x, y);
  const len = Math.sqrt(nx * nx + ny * ny + 1.0);
  return {
    x: -nx / len,
    y: -ny / len,
    z: 1.0 / len,
  };
}

/**
 * Computes illumination intensity I from surface normal
 */
export function computeIllumination(normal: Vector3): number {
  const dot = normal.x * LX + normal.y * LY + normal.z * LZ;
  return IA + ID * Math.max(0, dot);
}

/**
 * Computes shaded pixel color
 */
export function shadePixel(baseRgb: ColorRgb, intensity: number): ColorRgb {
  return {
    r: Math.min(255, Math.max(0, Math.round(baseRgb.r * intensity))),
    g: Math.min(255, Math.max(0, Math.round(baseRgb.g * intensity))),
    b: Math.min(255, Math.max(0, Math.round(baseRgb.b * intensity))),
  };
}

/**
 * High-performance Canvas2D cushion treemap renderer using separable 1D lookups
 */
export function renderCushionTreemap(
  ctx: CanvasRenderingContext2D,
  items: TreemapLayoutItem[],
  colorMap: Map<string, string>,
  width: number,
  height: number,
  hoveredNodeId: number | null = null,
  selectedNodeId: number | null = null,
  isolatedExtension: string | null = null
) {
  if (width <= 0 || height <= 0) return;

  ctx.fillStyle = "#0F172A";
  ctx.fillRect(0, 0, width, height);

  for (const item of items) {
    const rx = Math.round(item.rect.x);
    const ry = Math.round(item.rect.y);
    const rw = Math.round(item.rect.w);
    const rh = Math.round(item.rect.h);

    if (rw <= 0 || rh <= 0) continue;

    const baseHex = colorMap.get(item.extension) || "#64748B";
    let baseRgb = hexToRgb(baseHex);

    // If an extension is isolated, dim other extensions
    if (isolatedExtension && item.extension.toLowerCase() !== isolatedExtension.toLowerCase()) {
      baseRgb = {
        r: Math.round(baseRgb.r * 0.25),
        g: Math.round(baseRgb.g * 0.25),
        b: Math.round(baseRgb.b * 0.25),
      };
    }

    const imgData = ctx.createImageData(rw, rh);
    const data32 = new Uint32Array(imgData.data.buffer);

    // Separable 1D precomputation of normal components
    const nxArr = new Float32Array(rw);
    const nyArr = new Float32Array(rh);

    for (let x = 0; x < rw; x++) {
      const px = rx + x + 0.5;
      let nx = 0;
      for (let c = 0; c < item.cushions.length; c++) {
        const cs = item.cushions[c];
        const cw = cs.x2 - cs.x1;
        if (cw > 0) {
          const hk = H0 * Math.pow(F, cs.depth);
          nx += -4 * hk * (2 * px - (cs.x1 + cs.x2)) / (cw * cw);
        }
      }
      nxArr[x] = nx;
    }

    for (let y = 0; y < rh; y++) {
      const py = ry + y + 0.5;
      let ny = 0;
      for (let c = 0; c < item.cushions.length; c++) {
        const cs = item.cushions[c];
        const ch = cs.y2 - cs.y1;
        if (ch > 0) {
          const hk = H0 * Math.pow(F, cs.depth);
          ny += -4 * hk * (2 * py - (cs.y1 + cs.y2)) / (ch * ch);
        }
      }
      nyArr[y] = ny;
    }

    let ptr = 0;
    for (let y = 0; y < rh; y++) {
      const ny = nyArr[y];
      for (let x = 0; x < rw; x++) {
        const nx = nxArr[x];
        const len = Math.sqrt(nx * nx + ny * ny + 1.0);
        const normX = -nx / len;
        const normY = -ny / len;
        const normZ = 1.0 / len;

        const dot = normX * LX + normY * LY + normZ * LZ;
        const intensity = IA + ID * Math.max(0, dot);

        const r = Math.min(255, Math.max(0, Math.round(baseRgb.r * intensity)));
        const g = Math.min(255, Math.max(0, Math.round(baseRgb.g * intensity)));
        const b = Math.min(255, Math.max(0, Math.round(baseRgb.b * intensity)));

        // Little-endian RGBA packing (0xAABBGGRR)
        data32[ptr++] = (255 << 24) | (b << 16) | (g << 8) | r;
      }
    }

    ctx.putImageData(imgData, rx, ry);

    // Draw borders & selection indicators
    if (item.nodeId === selectedNodeId) {
      ctx.strokeStyle = "#38BDF8";
      ctx.lineWidth = 2;
      ctx.strokeRect(rx + 1, ry + 1, rw - 2, rh - 2);
    } else if (item.nodeId === hoveredNodeId) {
      ctx.strokeStyle = "#F8FAFC";
      ctx.lineWidth = 1.5;
      ctx.strokeRect(rx + 0.5, ry + 0.5, rw - 1, rh - 1);
    } else {
      ctx.strokeStyle = "rgba(15, 23, 42, 0.4)";
      ctx.lineWidth = 1;
      ctx.strokeRect(rx + 0.5, ry + 0.5, rw - 1, rh - 1);
    }
  }
}
