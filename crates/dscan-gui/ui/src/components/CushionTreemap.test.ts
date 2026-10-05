import "../test-dom";
import { beforeEach, describe, expect, test } from "bun:test";
import { scanStore } from "../state/scanStore";
import type { TreemapNode } from "../treemap/squarify";
import { CushionTreemap } from "./CushionTreemap";

describe("CushionTreemap Component", () => {
  beforeEach(() => {
    scanStore.nodeMap.clear();
    scanStore.treeNodes = [];
    scanStore.visualRootId = 0;
    scanStore.breadcrumb = [0];
  });

  test("initializes without error and renders placeholder", () => {
    const treemap = new CushionTreemap();
    expect(treemap.getElement()).toBeDefined();
    expect(treemap.getLayoutItems()).toEqual([]);
  });

  test("generates layout items from synthetic tree hierarchy", () => {
    const nodes: TreemapNode[] = [
      {
        id: 0,
        parent_id: 0,
        name: "root",
        total_bytes: 1_000_000,
        direct_bytes: 0,
        rel_depth: 0,
        is_dir: true,
        extension: "",
        children_ids: [1, 2],
      },
      {
        id: 1,
        parent_id: 0,
        name: "file1.mp4",
        total_bytes: 600_000,
        direct_bytes: 600_000,
        rel_depth: 1,
        is_dir: false,
        extension: ".mp4",
        children_ids: [],
      },
      {
        id: 2,
        parent_id: 0,
        name: "file2.rs",
        total_bytes: 400_000,
        direct_bytes: 400_000,
        rel_depth: 1,
        is_dir: false,
        extension: ".rs",
        children_ids: [],
      },
    ];

    scanStore.setTreeData(nodes, "/root");
    scanStore.setExtensions([
      { extension: ".mp4", totalBytes: 600_000, fileCount: 1, percentageOfTotal: 60 },
      { extension: ".rs", totalBytes: 400_000, fileCount: 1, percentageOfTotal: 40 },
    ]);

    const treemap = new CushionTreemap();
    treemap.setSize(800, 600);

    treemap.render();
    const items = treemap.getLayoutItems();
    expect(items.length).toBe(2);
    expect(items[0].nodeId).toBe(1);
    expect(items[1].nodeId).toBe(2);
  });
});
