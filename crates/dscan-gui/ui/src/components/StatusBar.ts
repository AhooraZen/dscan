/**
 * Bottom Status Bar Component.
 * Displays scan state pill, tail-truncated path, real-time rates, and drive usage bar.
 */

import { scanStore } from "../state/scanStore";
import { formatBytes } from "./DirectoryTree";

export function truncatePath(path: string, maxLength: number = 45): string {
  if (!path || path.length <= maxLength) return path || "";
  const parts = path.split("/");
  if (parts.length <= 2) {
    return "..." + path.slice(-(maxLength - 3));
  }
  let result = parts[parts.length - 1];
  for (let i = parts.length - 2; i >= 0; i--) {
    const candidate = parts[i] + "/" + result;
    if (candidate.length + 4 > maxLength) {
      return ".../" + result;
    }
    result = candidate;
  }
  return ".../" + result;
}

export function formatDuration(millis: number): string {
  const totalSeconds = Math.floor(millis / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
}

export class StatusBar {
  private container: HTMLElement;

  constructor() {
    this.container = document.createElement("div");
    this.container.className =
      "h-7 w-full bg-surface-card border-t border-surface-border px-3 flex items-center justify-between text-[11px] text-text-muted select-none";

    this.render();
    scanStore.subscribe(() => this.render());
  }

  getElement(): HTMLElement {
    return this.container;
  }

  public render() {
    const isScanning = scanStore.isScanning;
    const isPaused = scanStore.isPaused;
    const isComplete = scanStore.progress.isComplete;
    const p = scanStore.progress;

    // 1. Status Pill
    let pillText = "Ready";
    let pillDotColor = "bg-slate-400";
    let pillBg = "bg-surface-panel text-slate-300";

    if (isPaused) {
      pillText = "Paused";
      pillDotColor = "bg-accent-yellow";
      pillBg = "bg-accent-yellow/20 text-accent-yellow border border-accent-yellow/40";
    } else if (isScanning) {
      pillText = "Scanning";
      pillDotColor = "bg-accent-cyan animate-pulse";
      pillBg = "bg-accent-cyan/20 text-accent-cyan border border-accent-cyan/40";
    } else if (isComplete && p.totalFiles > 0) {
      pillText = "Finished";
      pillDotColor = "bg-emerald-400";
      pillBg = "bg-emerald-500/20 text-emerald-300 border border-emerald-500/40";
    }

    const pathDisplay = truncatePath(p.currentPath, 50);

    // 2. Metrics (Files, Rate, Space, Elapsed)
    const filesStr = p.totalFiles.toLocaleString();
    const filesSecStr = p.filesPerSec > 0 ? `${Math.round(p.filesPerSec).toLocaleString()}/s` : "-";
    const bytesStr = formatBytes(p.totalBytes);
    const bytesSecStr = p.bytesPerSec > 0 ? `${formatBytes(p.bytesPerSec)}/s` : "-";
    const elapsedStr = formatDuration(p.elapsedMillis);

    // 3. Drive usage (from selected or root drive if available)
    const currentDrive = scanStore.drives[0];
    let driveUsageHtml = "";
    if (currentDrive && currentDrive.total_bytes > 0) {
      const usedBytes = currentDrive.total_bytes - currentDrive.free_bytes;
      const pct = (usedBytes / currentDrive.total_bytes) * 100;
      driveUsageHtml = `
        <div class="flex items-center space-x-2 pl-3 border-l border-surface-border/60">
          <span>Drive:</span>
          <div class="w-16 h-2 bg-surface-panel rounded-full overflow-hidden border border-surface-border">
            <div class="h-full bg-accent-blue" style="width: ${pct.toFixed(0)}%"></div>
          </div>
          <span class="tabular-nums">${pct.toFixed(0)}% (${formatBytes(currentDrive.total_bytes)})</span>
        </div>
      `;
    }

    this.container.innerHTML = `
      <div class="flex items-center space-x-3 overflow-hidden">
        <div class="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-semibold tracking-wide uppercase ${pillBg}">
          <span class="w-1.5 h-1.5 rounded-full mr-1.5 ${pillDotColor}"></span>
          ${pillText}
        </div>
        <div class="truncate max-w-[320px] font-mono text-slate-300" title="${p.currentPath}">
          ${pathDisplay}
        </div>
      </div>

      <div class="flex items-center space-x-4">
        <div class="flex items-center space-x-3 tabular-nums">
          <span>Files: <strong class="text-text-primary font-medium">${filesStr}</strong> <span class="text-text-muted">(${filesSecStr})</span></span>
          <span>Size: <strong class="text-text-primary font-medium">${bytesStr}</strong> <span class="text-text-muted">(${bytesSecStr})</span></span>
          <span>Time: <strong class="text-text-primary font-medium">${elapsedStr}</strong></span>
        </div>
        ${driveUsageHtml}
      </div>
    `;
  }
}
