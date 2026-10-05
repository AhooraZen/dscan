import { describe, expect, test } from "bun:test";
import {
  CATEGORICAL_OKLCH,
  PANEL_BG,
  PaletteManager,
  contrastRatio,
  oklabDeltaE,
  oklchToRgb,
} from "./palette";

describe("High-Contrast OKLCH Palette", () => {
  test("All 16 colors meet WCAG AA non-text contrast (>= 3.0:1) against panel surface #1E293B", () => {
    for (let i = 0; i < CATEGORICAL_OKLCH.length; i++) {
      const rgb = oklchToRgb(CATEGORICAL_OKLCH[i]);
      const ratio = contrastRatio(rgb, PANEL_BG);
      expect(ratio).toBeGreaterThanOrEqual(3.0);
    }
  });

  test("No two top-10 colors have Delta-E < 15 (maximally distinct separation)", () => {
    const top10 = CATEGORICAL_OKLCH.slice(0, 10);
    for (let i = 0; i < top10.length; i++) {
      for (let j = i + 1; j < top10.length; j++) {
        const delta = oklabDeltaE(top10[i], top10[j]);
        expect(delta).toBeGreaterThanOrEqual(15);
      }
    }
  });

  test("PaletteManager assigns colors deterministically and uses overflow color after 16", () => {
    const pm = new PaletteManager();
    const c1 = pm.getColor(".mp4");
    const c2 = pm.getColor(".mp4");
    expect(c1).toBe(c2);

    const extensions = [
      ".mp4", ".iso", ".tar", ".gz", ".rs", ".zip", ".bin", ".dat",
      ".mkv", ".png", ".jpg", ".webp", ".pdf", ".docx", ".xlsx", ".csv",
      ".extra1", ".extra2"
    ];

    const colors = extensions.map(e => pm.getColor(e));
    expect(colors[0]).toBe(c1);
    expect(colors[16]).toBe("#64748B"); // overflow
    expect(colors[17]).toBe("#64748B"); // overflow
  });
});
