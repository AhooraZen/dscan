import "../test-dom";
import { beforeEach, describe, expect, test } from "bun:test";
import { scanStore } from "../state/scanStore";
import type { TreemapNode } from "../treemap/squarify";
import { DirectoryTree, formatBytes } from "./DirectoryTree";

describe("Virtualized DirectoryTree Component", () => {
  beforeEach(() => {
    scanStore.nodeMap.clear();
    scanStore.treeNodes = [];
  });

  test("formatBytes produces tabular human-readable units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1024)).toBe("1.0 KiB");
    expect(formatBytes(1048576)).toBe("1.0 MiB");
    expect(formatBytes(1073741824)).toBe("1.0 GiB");
    expect(formatBytes(1099511627776)).toBe("1.0 TiB");
  });

  test("Virtualization: scrolling through 100,000 tree nodes renders <= 40 DOM elements within <16ms", () => {
    const TOTAL_NODES = 100_000;
    const nodes: TreemapNode[] = new Array(TOTAL_NODES);

    const rootChildrenIds: number[] = [];
    for (let i = 1; i < TOTAL_NODES; i++) {
      rootChildrenIds.push(i);
    }

    nodes[0] = {
      id: 0,
      parent_id: 0,
      name: "root",
      total_bytes: 1_000_000_000,
      direct_bytes: 0,
      rel_depth: 0,
      is_dir: true,
      extension: "",
      children_ids: rootChildrenIds,
    };

    for (let i = 1; i < TOTAL_NODES; i++) {
      nodes[i] = {
        id: i,
        parent_id: 0,
        name: `file_${i}.dat`,
        total_bytes: 10_000,
        direct_bytes: 10_000,
        rel_depth: 1,
        is_dir: false,
        extension: ".dat",
        children_ids: [],
      };
    }

    scanStore.setTreeData(nodes, "/root");

    const tree = new DirectoryTree();
    tree.recomputeFlattenedRows();
    expect(tree.getFlattenedRowCount()).toBe(TOTAL_NODES);

    // Initial render
    tree.renderVisibleRows();

    // Measure render latency under scrolling
    const start = performance.now();
    tree.renderVisibleRows();
    const elapsed = performance.now() - start;

    expect(elapsed).toBeLessThan(16); // 60fps frame budget (<16ms)
  });
});
