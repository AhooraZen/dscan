/**
 * High-Information-Density Virtualized Directory Tree View.
 * Renders 100,000+ hierarchical nodes smoothly using bounded DOM rows (<= 40 elements).
 */

import { scanStore } from "../state/scanStore";
import type { TreemapNode } from "../treemap/squarify";
import { PacmanAnimator } from "./Pacman";

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const clampedI = Math.min(i, units.length - 1);
  const val = bytes / Math.pow(1024, clampedI);
  return `${val.toFixed(clampedI === 0 ? 0 : 1)} ${units[clampedI]}`;
}

export interface FlattenedRow {
  node: TreemapNode;
  depth: number;
  isExpanded: boolean;
  hasChildren: boolean;
  parentBytes: number;
}

export class DirectoryTree {
  private container: HTMLElement;
  private scrollContainer: HTMLElement;
  private spacer: HTMLElement;
  private content: HTMLElement;
  private header: HTMLElement;

  private rowHeight = 24;
  private overscan = 5;

  private expandedIds = new Set<number>();
  private flattenedRows: FlattenedRow[] = [];
  private pacman = new PacmanAnimator(28, 16);

  constructor() {
    this.container = document.createElement("div");
    this.container.className = "flex flex-col h-full w-full bg-surface-panel select-none overflow-hidden text-xs";

    // Header Row
    this.header = document.createElement("div");
    this.header.className =
      "grid grid-cols-[1fr_80px_90px_60px_36px] items-center px-2 py-1 bg-surface-card border-b border-surface-border text-text-muted font-medium uppercase tracking-wider text-[11px]";
    this.header.innerHTML = `
      <div class="truncate">Name</div>
      <div class="text-right pr-2">% Subtree</div>
      <div class="text-right pr-2">Size</div>
      <div class="text-right pr-2">Items</div>
      <div class="text-center">State</div>
    `;
    this.container.appendChild(this.header);

    // Scroll Viewport
    this.scrollContainer = document.createElement("div");
    this.scrollContainer.className = "flex-1 overflow-y-auto relative outline-none focus:ring-1 focus:ring-accent-blue";
    this.scrollContainer.tabIndex = 0;

    this.spacer = document.createElement("div");
    this.spacer.className = "w-full pointer-events-none";

    this.content = document.createElement("div");
    this.content.className = "absolute top-0 left-0 w-full";

    this.scrollContainer.appendChild(this.spacer);
    this.scrollContainer.appendChild(this.content);
    this.container.appendChild(this.scrollContainer);

    this.setupEvents();
    this.expandedIds.add(scanStore.visualRootId ?? 0);
    this.recomputeFlattenedRows();
    scanStore.subscribe(() => this.onStoreUpdate());
  }

  getElement(): HTMLElement {
    return this.container;
  }

  private setupEvents() {
    this.scrollContainer.addEventListener("scroll", () => this.renderVisibleRows());
    if (typeof window !== "undefined") {
      window.addEventListener("resize", () => this.renderVisibleRows());
    }

    this.scrollContainer.addEventListener("keydown", (e) => {
      this.handleKeyDown(e);
    });

    this.content.addEventListener("click", (e) => {
      const rowEl = (e.target as HTMLElement).closest("[data-node-id]") as HTMLElement | null;
      if (!rowEl) return;
      const nodeId = parseInt(rowEl.dataset.nodeId || "", 10);
      if (isNaN(nodeId)) return;

      const toggleCaret = (e.target as HTMLElement).closest("[data-toggle-caret]");
      if (toggleCaret) {
        this.toggleExpand(nodeId);
      } else {
        scanStore.selectNode(nodeId);
      }
    });

    this.content.addEventListener("dblclick", (e) => {
      const rowEl = (e.target as HTMLElement).closest("[data-node-id]") as HTMLElement | null;
      if (!rowEl) return;
      const nodeId = parseInt(rowEl.dataset.nodeId || "", 10);
      if (isNaN(nodeId)) return;

      const node = scanStore.nodeMap.get(nodeId);
      if (node && node.is_dir) {
        scanStore.drillDown(nodeId);
      }
    });
  }

  private handleKeyDown(e: KeyboardEvent) {
    const selId = scanStore.selectedNodeId;
    if (selId === null && this.flattenedRows.length > 0) {
      scanStore.selectNode(this.flattenedRows[0].node.id);
      return;
    }

    const currentIdx = this.flattenedRows.findIndex((r) => r.node.id === selId);
    if (currentIdx === -1) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (currentIdx + 1 < this.flattenedRows.length) {
        const nextId = this.flattenedRows[currentIdx + 1].node.id;
        scanStore.selectNode(nextId);
        this.scrollIntoView(nextId);
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (currentIdx > 0) {
        const prevId = this.flattenedRows[currentIdx - 1].node.id;
        scanStore.selectNode(prevId);
        this.scrollIntoView(prevId);
      }
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      const row = this.flattenedRows[currentIdx];
      if (row.hasChildren && !this.expandedIds.has(row.node.id)) {
        this.toggleExpand(row.node.id);
      }
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      const row = this.flattenedRows[currentIdx];
      if (this.expandedIds.has(row.node.id)) {
        this.toggleExpand(row.node.id);
      } else if (row.node.parent_id !== row.node.id) {
        scanStore.selectNode(row.node.parent_id);
        this.scrollIntoView(row.node.parent_id);
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      const row = this.flattenedRows[currentIdx];
      if (row.node.is_dir) {
        scanStore.drillDown(row.node.id);
      }
    }
  }

  private toggleExpand(nodeId: number) {
    if (this.expandedIds.has(nodeId)) {
      this.expandedIds.delete(nodeId);
    } else {
      this.expandedIds.add(nodeId);
    }
    this.recomputeFlattenedRows();
    this.renderVisibleRows();
  }

  private scrollIntoView(nodeId: number) {
    const idx = this.flattenedRows.findIndex((r) => r.node.id === nodeId);
    if (idx === -1) return;
    const targetTop = idx * this.rowHeight;
    const targetBottom = targetTop + this.rowHeight;
    const scrollTop = this.scrollContainer.scrollTop;
    const viewHeight = this.scrollContainer.clientHeight;

    if (targetTop < scrollTop) {
      this.scrollContainer.scrollTop = targetTop;
    } else if (targetBottom > scrollTop + viewHeight) {
      this.scrollContainer.scrollTop = targetBottom - viewHeight;
    }
  }

  private onStoreUpdate() {
    if (scanStore.isScanning) {
      this.pacman.start();
    } else {
      this.pacman.stop();
    }

    // Auto-expand root by default
    if (this.expandedIds.size === 0 && scanStore.visualRootId !== undefined) {
      this.expandedIds.add(scanStore.visualRootId);
    }

    this.recomputeFlattenedRows();
    this.renderVisibleRows();
  }

  /**
   * Flattens hierarchical tree into visible list based on expanded set
   */
  public recomputeFlattenedRows() {
    const rows: FlattenedRow[] = [];
    const rootNode = scanStore.nodeMap.get(scanStore.visualRootId);
    if (!rootNode) {
      this.flattenedRows = [];
      this.spacer.style.height = "0px";
      return;
    }

    const recurse = (curr: TreemapNode, depth: number, parentBytes: number) => {
      const isExpanded = this.expandedIds.has(curr.id);
      const hasChildren = curr.children_ids.length > 0;

      rows.push({
        node: curr,
        depth,
        isExpanded,
        hasChildren,
        parentBytes: parentBytes > 0 ? parentBytes : curr.total_bytes,
      });

      if (isExpanded && hasChildren) {
        // Sort children by size descending
        const children = curr.children_ids
          .map((id) => scanStore.nodeMap.get(id))
          .filter((n): n is TreemapNode => Boolean(n));

        children.sort((a, b) => b.total_bytes - a.total_bytes);

        for (const child of children) {
          recurse(child, depth + 1, curr.total_bytes);
        }
      }
    };

    recurse(rootNode, 0, rootNode.total_bytes);
    this.flattenedRows = rows;
    this.spacer.style.height = `${rows.length * this.rowHeight}px`;
  }

  public getFlattenedRowCount(): number {
    return this.flattenedRows.length;
  }

  public getRenderedRowCount(): number {
    return this.content.children.length;
  }

  /**
   * Virtualized window calculation and row injection
   */
  public renderVisibleRows() {
    const totalRows = this.flattenedRows.length;
    if (totalRows === 0) {
      this.content.innerHTML = `<div class="p-4 text-center text-text-muted">No files scanned yet. Click "Scan Drive..." to begin.</div>`;
      return;
    }

    const scrollTop = this.scrollContainer.scrollTop;
    const containerHeight = this.scrollContainer.clientHeight || 400;

    const startIndex = Math.max(0, Math.floor(scrollTop / this.rowHeight) - this.overscan);
    const endIndex = Math.min(totalRows, Math.ceil((scrollTop + containerHeight) / this.rowHeight) + this.overscan);

    this.content.style.transform = `translateY(${startIndex * this.rowHeight}px)`;

    const selectedId = scanStore.selectedNodeId;
    let html = "";

    for (let i = startIndex; i < endIndex; i++) {
      const row = this.flattenedRows[i];
      const node = row.node;
      const isSelected = node.id === selectedId;

      const pct = row.parentBytes > 0 ? (node.total_bytes / row.parentBytes) * 100 : 100;
      const pctFormatted = pct.toFixed(1) + "%";
      const sizeStr = formatBytes(node.total_bytes);
      const itemsCount = node.children_ids.length > 0 ? node.children_ids.length.toLocaleString() : "1";

      const indentPx = row.depth * 14;

      // Caret icon
      let caretHtml = `<span class="w-3.5 inline-block"></span>`;
      if (row.hasChildren) {
        caretHtml = `
          <button data-toggle-caret="true" class="w-3.5 h-3.5 inline-flex items-center justify-center text-text-muted hover:text-text-primary">
            <svg class="w-2.5 h-2.5 transform transition-transform ${row.isExpanded ? "rotate-90" : ""}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="m9 18 6-6-6-6"/></svg>
          </button>
        `;
      }

      // Folder / File Icon (Clean SVG, NO emoji)
      const iconHtml = node.is_dir
        ? `<svg class="w-3.5 h-3.5 text-accent-yellow inline-block mr-1 flex-shrink-0" viewBox="0 0 24 24" fill="currentColor"><path d="M20 18a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2h16Z"/></svg>`
        : `<svg class="w-3.5 h-3.5 text-accent-cyan inline-block mr-1 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>`;

      const selectedClass = isSelected
        ? "bg-accent-blue/30 text-text-primary border-l-2 border-accent-cyan"
        : "hover:bg-surface-hover text-slate-300";

      html += `
        <div data-node-id="${node.id}" class="grid grid-cols-[1fr_80px_90px_60px_36px] items-center h-[24px] px-2 cursor-pointer border-b border-surface-border/40 ${selectedClass}">
          <div class="flex items-center truncate" style="padding-left: ${indentPx}px">
            ${caretHtml}
            ${iconHtml}
            <span class="truncate ml-0.5" title="${node.name}">${node.name}</span>
          </div>
          <div class="flex items-center justify-end pr-2">
            <div class="w-10 h-1.5 bg-surface-card rounded-full overflow-hidden mr-1.5">
              <div class="h-full bg-accent-cyan" style="width: ${Math.min(100, Math.max(2, pct))}%"></div>
            </div>
            <span class="tabular-nums text-[10px] text-text-muted w-8 text-right">${pctFormatted}</span>
          </div>
          <div class="text-right pr-2 tabular-nums text-text-primary">${sizeStr}</div>
          <div class="text-right pr-2 tabular-nums text-text-muted">${itemsCount}</div>
          <div class="flex items-center justify-center pacman-slot-${node.id}">
            ${node.id === 0 && scanStore.isScanning ? "" : ""}
          </div>
        </div>
      `;
    }

    this.content.innerHTML = html;

    // Attach active pacman canvas to scanning root if active
    if (scanStore.isScanning && startIndex === 0) {
      const slot = this.content.querySelector(".pacman-slot-0");
      if (slot && !slot.contains(this.pacman.getElement())) {
        slot.appendChild(this.pacman.getElement());
      }
    }
  }
}
