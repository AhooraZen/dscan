import { type ExtensionStat, scanStore } from "./scanStore";
import type { TreemapNode } from "../treemap/squarify";

// Safe invoke wrapper supporting both Tauri v2 environment and browser mock
async function safeInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      return await invoke<T>(cmd, args);
    } catch (err) {
      console.error(`Tauri command ${cmd} failed:`, err);
      throw err;
    }
  }

  // Browser mock fallback for unit tests and local vite dev
  console.warn(`[Browser Mock] Invoking ${cmd}`, args);
  if (cmd === "poll_progress") {
    return {
      total_bytes: 42_500_000_000,
      total_files: 142_800,
      active_workers: 16,
      current_path: "/home/ahoura/Projects/dscan",
      elapsed_millis: 1250,
      files_per_sec: 114_240,
      bytes_per_sec: 34_000_000_000,
      is_complete: true,
      is_paused: false,
    } as unknown as T;
  }
  if (cmd === "get_treemap_data") {
    return [
      {
        id: 0,
        parent_id: 0,
        name: "dscan",
        total_bytes: 42500000000,
        direct_bytes: 0,
        rel_depth: 0,
        is_dir: true,
        extension: "",
        children_ids: [1, 2],
      },
      {
        id: 1,
        parent_id: 0,
        name: "target",
        total_bytes: 32000000000,
        direct_bytes: 0,
        rel_depth: 1,
        is_dir: true,
        extension: "",
        children_ids: [3, 4],
      },
      {
        id: 2,
        parent_id: 0,
        name: "crates",
        total_bytes: 10500000000,
        direct_bytes: 10500000000,
        rel_depth: 1,
        is_dir: false,
        extension: ".rs",
        children_ids: [],
      },
      {
        id: 3,
        parent_id: 1,
        name: "dscan_binary",
        total_bytes: 20000000000,
        direct_bytes: 20000000000,
        rel_depth: 2,
        is_dir: false,
        extension: ".bin",
        children_ids: [],
      },
      {
        id: 4,
        parent_id: 1,
        name: "cache.tar",
        total_bytes: 12000000000,
        direct_bytes: 12000000000,
        rel_depth: 2,
        is_dir: false,
        extension: ".tar",
        children_ids: [],
      },
    ] as unknown as T;
  }
  if (cmd === "get_extension_legend") {
    return [
      { extension: ".bin", total_bytes: 20000000000, file_count: 12, percentage_of_total: 47.1 },
      { extension: ".tar", total_bytes: 12000000000, file_count: 8, percentage_of_total: 28.2 },
      { extension: ".rs", total_bytes: 10500000000, file_count: 420, percentage_of_total: 24.7 },
    ] as unknown as T;
  }
  if (cmd === "get_system_drives") {
    return [
      { path: "/home", name: "Home (/home)", total_bytes: 1000000000000, free_bytes: 450000000000 },
      { path: "/", name: "Root (/)", total_bytes: 1000000000000, free_bytes: 450000000000 },
    ] as unknown as T;
  }
  return null as unknown as T;
}

export class ScanController {
  private pollTimer: ReturnType<typeof setInterval> | null = null;

  async loadDrives() {
    try {
      const drives = await safeInvoke<{ path: string; name: string; total_bytes: number; free_bytes: number }[]>("get_system_drives");
      if (drives && Array.isArray(drives)) {
        scanStore.drives = drives;
        scanStore.notify();
      }
    } catch (err) {
      console.error("Failed to load drives:", err);
    }
  }

  async startScan(path: string, threads?: number) {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }

    scanStore.setScanning(true);
    scanStore.setPaused(false);
    scanStore.updateProgress({
      totalBytes: 0,
      totalFiles: 0,
      currentPath: path,
      elapsedMillis: 0,
      filesPerSec: 0,
      bytesPerSec: 0,
      isComplete: false,
      isPaused: false,
    });

    try {
      await safeInvoke("start_scan", { path, threads });
    } catch (err) {
      console.error("Failed to start scan:", err);
      scanStore.setScanning(false);
      return;
    }

    // 100ms atomic polling loop
    this.pollTimer = setInterval(async () => {
      try {
        interface RawProgress {
          total_bytes: number;
          total_files: number;
          active_workers: number;
          current_path: string;
          elapsed_millis: number;
          files_per_sec: number;
          bytes_per_sec: number;
          is_complete: boolean;
          is_paused: boolean;
        }

        const raw = await safeInvoke<RawProgress>("poll_progress");
        if (!raw) return;

        scanStore.updateProgress({
          totalBytes: raw.total_bytes,
          totalFiles: raw.total_files,
          activeWorkers: raw.active_workers,
          currentPath: raw.current_path,
          elapsedMillis: raw.elapsed_millis,
          filesPerSec: raw.files_per_sec,
          bytesPerSec: raw.bytes_per_sec,
          isComplete: raw.is_complete,
          isPaused: raw.is_paused,
        });

        if (raw.is_complete) {
          if (this.pollTimer) {
            clearInterval(this.pollTimer);
            this.pollTimer = null;
          }
          await this.finishScan(path);
        }
      } catch (err) {
        console.error("Poll progress error:", err);
      }
    }, 100);
  }

  async finishScan(rootPath: string) {
    scanStore.setScanning(false);
    try {
      const nodes = await safeInvoke<TreemapNode[]>("get_treemap_data", { depth: 6, max_nodes: 5000 });
      if (nodes && Array.isArray(nodes)) {
        scanStore.setTreeData(nodes, rootPath);
      }

      interface RawExtension {
        extension: string;
        total_bytes: number;
        file_count: number;
        percentage_of_total: number;
      }
      const rawExts = await safeInvoke<RawExtension[]>("get_extension_legend", { limit: 30 });
      if (rawExts && Array.isArray(rawExts)) {
        const stats: ExtensionStat[] = rawExts.map(e => ({
          extension: e.extension,
          totalBytes: e.total_bytes,
          fileCount: e.file_count,
          percentageOfTotal: e.percentage_of_total,
        }));
        scanStore.setExtensions(stats);
      }
    } catch (err) {
      console.error("Failed to load completed scan data:", err);
    }
  }

  async cancelScan() {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    scanStore.setScanning(false);
    try {
      await safeInvoke("cancel_scan");
    } catch (err) {
      console.error("Cancel scan error:", err);
    }
  }

  async togglePause() {
    try {
      const isPaused = await safeInvoke<boolean>("pause_scan");
      scanStore.setPaused(isPaused);
    } catch (err) {
      console.error("Pause scan error:", err);
    }
  }

  async openInFileManager(path: string) {
    try {
      await safeInvoke("open_in_file_manager", { path });
    } catch (err) {
      console.error("Open file manager error:", err);
    }
  }
}

export const scanController = new ScanController();
