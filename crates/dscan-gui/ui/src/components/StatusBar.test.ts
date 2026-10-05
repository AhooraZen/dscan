import "../test-dom";
import { describe, expect, test } from "bun:test";
import { scanStore } from "../state/scanStore";
import { formatDuration, StatusBar, truncatePath } from "./StatusBar";

describe("StatusBar Component", () => {
  test("truncatePath correctly preserves tail segments", () => {
    expect(truncatePath("/short/path", 40)).toBe("/short/path");
    expect(
      truncatePath("/usr/local/share/data/very/long/nested/path/to/scan/deep/file.txt", 30)
    ).toContain("file.txt");
    expect(
      truncatePath("/usr/local/share/data/very/long/nested/path/to/scan/deep/file.txt", 30).startsWith("...")
    ).toBe(true);
  });

  test("formatDuration formats seconds and minutes padded", () => {
    expect(formatDuration(0)).toBe("00:00");
    expect(formatDuration(5000)).toBe("00:05");
    expect(formatDuration(65000)).toBe("01:05");
    expect(formatDuration(3600000)).toBe("60:00");
  });

  test("renders status pill states", () => {
    const sb = new StatusBar();
    expect(sb.getElement()).toBeDefined();

    scanStore.updateProgress({
      totalBytes: 1048576,
      totalFiles: 42,
      currentPath: "/home/user/test",
      elapsedMillis: 10000,
      filesPerSec: 4.2,
      bytesPerSec: 104857.6,
      isComplete: false,
    });
    expect(sb.getElement().innerHTML).toContain("1.0 MiB");
  });
});
