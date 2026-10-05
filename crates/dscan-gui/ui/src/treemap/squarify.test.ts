import { describe, expect, test } from "bun:test";
import {
  type Rect,
  type TreemapNode,
  buildTreemapLayout,
  squarifyChildren,
} from "./squarify";

describe("Squarified Treemap Layout Engine", () => {
  test("test_squarified_aspect_ratios: generated rectangles avoid thin needles (aspect ratio <= 4.0)", () => {
    // Generate random items with skewed sizes (like typical filesystems: few huge, many small)
    const items = [
      { id: 1, val: 50000 },
      { id: 2, val: 32000 },
      { id: 3, val: 18000 },
      { id: 4, val: 12000 },
      { id: 5, val: 8000 },
      { id: 6, val: 4000 },
      { id: 7, val: 2500 },
      { id: 8, val: 1000 },
      { id: 9, val: 800 },
      { id: 10, val: 200 },
    ].map(it => ({
      node: {
        id: it.id,
        parent_id: 0,
        name: `file_${it.id}`,
        total_bytes: it.val,
        direct_bytes: it.val,
        rel_depth: 1,
        is_dir: false,
        extension: ".bin",
        children_ids: [],
      },
      value: it.val,
    }));

    const bounds: Rect = { x: 0, y: 0, w: 1000, h: 600 };
    const layout = squarifyChildren(items, bounds);

    expect(layout.length).toBe(items.length);

    let maxAspectRatio = 0;
    for (const item of layout) {
      const ar = Math.max(item.rect.w / item.rect.h, item.rect.h / item.rect.w);
      if (ar > maxAspectRatio) {
        maxAspectRatio = ar;
      }
      expect(ar).toBeLessThanOrEqual(4.0);
    }
  });

  test("buildTreemapLayout handles multi-level nesting and produces valid cushion hierarchies", () => {
    const nodes: TreemapNode[] = [
      {
        id: 0,
        parent_id: 0,
        name: "root",
        total_bytes: 1000,
        direct_bytes: 0,
        rel_depth: 0,
        is_dir: true,
        extension: "",
        children_ids: [1, 2],
      },
      {
        id: 1,
        parent_id: 0,
        name: "dir_a",
        total_bytes: 600,
        direct_bytes: 0,
        rel_depth: 1,
        is_dir: true,
        extension: "",
        children_ids: [3, 4],
      },
      {
        id: 2,
        parent_id: 0,
        name: "dir_b",
        total_bytes: 400,
        direct_bytes: 400,
        rel_depth: 1,
        is_dir: false,
        extension: ".mp4",
        children_ids: [],
      },
      {
        id: 3,
        parent_id: 1,
        name: "file_a1.iso",
        total_bytes: 400,
        direct_bytes: 400,
        rel_depth: 2,
        is_dir: false,
        extension: ".iso",
        children_ids: [],
      },
      {
        id: 4,
        parent_id: 1,
        name: "file_a2.rs",
        total_bytes: 200,
        direct_bytes: 200,
        rel_depth: 2,
        is_dir: false,
        extension: ".rs",
        children_ids: [],
      },
    ];

    const bounds: Rect = { x: 0, y: 0, w: 800, h: 500 };
    const leaves = buildTreemapLayout(nodes, 0, bounds, 4);

    expect(leaves.length).toBe(3); // file_a1.iso, file_a2.rs, dir_b

    // All leaves have cushions
    for (const leaf of leaves) {
      expect(leaf.cushions.length).toBeGreaterThanOrEqual(1);
    }
  });
});
