/**
 * Extension Breakdown Legend Component (WinDirStat top-right pane).
 * Lists file types sorted descending by size, with categorical color swatches.
 */

import { scanStore } from "../state/scanStore";
import { formatBytes } from "./DirectoryTree";

export class ExtensionLegend {
  private container: HTMLElement;
  private header: HTMLElement;
  private scrollContainer: HTMLElement;

  constructor() {
    this.container = document.createElement("div");
    this.container.className = "flex flex-col h-full w-full bg-surface-panel select-none overflow-hidden text-xs";

    this.header = document.createElement("div");
    this.header.className =
      "grid grid-cols-[32px_1fr_80px_60px_50px] items-center px-2 py-1 bg-surface-card border-b border-surface-border text-text-muted font-medium uppercase tracking-wider text-[11px]";
    this.header.innerHTML = `
      <div class="text-center">Color</div>
      <div class="truncate pl-1">Extension</div>
      <div class="text-right pr-2">Size</div>
      <div class="text-right pr-2">Files</div>
      <div class="text-right pr-1">% Total</div>
    `;
    this.container.appendChild(this.header);

    this.scrollContainer = document.createElement("div");
    this.scrollContainer.className = "flex-1 overflow-y-auto outline-none";
    this.container.appendChild(this.scrollContainer);

    this.setupEvents();
    this.render();
    scanStore.subscribe(() => this.render());
  }

  getElement(): HTMLElement {
    return this.container;
  }

  private setupEvents() {
    this.scrollContainer.addEventListener("click", (e) => {
      const row = (e.target as HTMLElement).closest("[data-extension]") as HTMLElement | null;
      if (!row) return;
      const ext = row.dataset.extension;
      if (ext !== undefined) {
        scanStore.toggleIsolateExtension(ext);
      }
    });
  }

  public render() {
    const stats = scanStore.extensionStats;
    if (stats.length === 0) {
      this.scrollContainer.innerHTML = `
        <div class="p-4 text-center text-text-muted">
          No extension statistics available.
        </div>
      `;
      return;
    }

    const isolated = scanStore.isolatedExtension;
    let html = "";

    for (const stat of stats) {
      const color = scanStore.palette.getColor(stat.extension);
      const isIsolated = isolated === stat.extension;
      const rowClass = isIsolated
        ? "bg-accent-blue/30 border-l-2 border-accent-cyan text-text-primary"
        : isolated !== null
        ? "opacity-40 hover:opacity-100 hover:bg-surface-hover text-slate-400"
        : "hover:bg-surface-hover text-slate-300";

      const pctStr = stat.percentageOfTotal.toFixed(1) + "%";
      const sizeStr = formatBytes(stat.totalBytes);
      const countStr = stat.fileCount.toLocaleString();
      const displayName = stat.extension.length > 0 ? stat.extension : "[no ext]";

      html += `
        <div data-extension="${stat.extension}" class="grid grid-cols-[32px_1fr_80px_60px_50px] items-center h-[24px] px-2 cursor-pointer border-b border-surface-border/40 transition-colors ${rowClass}">
          <div class="flex items-center justify-center">
            <span class="w-3.5 h-3.5 rounded-sm shadow-sm inline-block" style="background-color: ${color}"></span>
          </div>
          <div class="truncate pl-1 font-mono font-medium text-text-primary" title="${displayName}">
            ${displayName}
          </div>
          <div class="text-right pr-2 tabular-nums">${sizeStr}</div>
          <div class="text-right pr-2 tabular-nums text-text-muted">${countStr}</div>
          <div class="text-right pr-1 tabular-nums text-accent-cyan">${pctStr}</div>
        </div>
      `;
    }

    this.scrollContainer.innerHTML = html;
  }
}
