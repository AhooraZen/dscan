import "../test-dom";
import { beforeEach, describe, expect, test } from "bun:test";
import { scanStore } from "../state/scanStore";
import { ExtensionLegend } from "./ExtensionLegend";

describe("ExtensionLegend Component", () => {
  beforeEach(() => {
    scanStore.extensionStats = [];
    scanStore.isolatedExtension = null;
  });

  test("renders empty placeholder when no stats exist", () => {
    const legend = new ExtensionLegend();
    expect(legend.getElement()).toBeDefined();
  });

  test("populates legend rows and isolates extension on click", () => {
    scanStore.setExtensions([
      { extension: ".mp4", totalBytes: 5_000_000, fileCount: 10, percentageOfTotal: 50.0 },
      { extension: ".rs", totalBytes: 3_000_000, fileCount: 100, percentageOfTotal: 30.0 },
      { extension: ".txt", totalBytes: 2_000_000, fileCount: 50, percentageOfTotal: 20.0 },
    ]);

    const legend = new ExtensionLegend();
    expect(legend.getElement()).toBeDefined();

    // Toggle isolation
    scanStore.toggleIsolateExtension(".mp4");
    expect(scanStore.isolatedExtension).toBe(".mp4");

    scanStore.toggleIsolateExtension(".mp4");
    expect(scanStore.isolatedExtension).toBeNull();
  });
});
