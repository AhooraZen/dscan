/**
 * Van Wijk Quadratic Cushion Treemap Component (Bottom Pane).
 * Renders hierarchical disk usage using Canvas2D illumination with drill-down and hover inspection.
 */

import { scanStore } from "../state/scanStore";
import { renderCushionTreemap } from "../treemap/cushion";
import {
  type Rect,
  type TreemapLayoutItem,
  buildTreemapLayout,
} from "../treemap/squarify";
import { formatBytes } from "./DirectoryTree";

export class CushionTreemap {
  private container: HTMLElement;
  private breadcrumbBar: HTMLElement;
  private canvasWrapper: HTMLElement;
  private canvas: HTMLCanvasElement;
  private tooltip: HTMLElement;
  private ctx: CanvasRenderingContext2D | null = null;

  private currentItems: TreemapLayoutItem[] = [];
  private width: number = 0;
  private height: number = 0;
  private resizeObserver: ResizeObserver | null = null;
  private animFrameId: number | null = null;

  constructor() {
    this.container = document.createElement("div");
    this.container.className =
      "flex flex-col h-full w-full bg-surface-panel select-none overflow-hidden relative";

    // 1. Breadcrumb Bar
    this.breadcrumbBar = document.createElement("div");
    this.breadcrumbBar.className =
      "h-7 w-full bg-surface-card border-b border-surface-border px-3 flex items-center justify-between text-xs text-text-muted";
    this.container.appendChild(this.breadcrumbBar);

    // 2. Canvas Wrapper
    this.canvasWrapper = document.createElement("div");
    this.canvasWrapper.className = "flex-1 w-full h-full relative overflow-hidden";

    this.canvas = document.createElement("canvas");
    this.canvas.className = "absolute inset-0 cursor-crosshair";
    this.canvasWrapper.appendChild(this.canvas);

    // 3. Hover Tooltip Card
    this.tooltip = document.createElement("div");
    this.tooltip.className =
      "absolute hidden pointer-events-none z-30 bg-surface-card/95 backdrop-blur border border-surface-border text-xs rounded-md shadow-2xl px-2.5 py-1.5 max-w-[280px]";
    this.canvasWrapper.appendChild(this.tooltip);

    this.container.appendChild(this.canvasWrapper);

    if (this.canvas.getContext) {
      this.ctx = this.canvas.getContext("2d", { alpha: false });
    }

    this.setupEvents();
    this.setupResizeObserver();
    this.renderBreadcrumbs();
    scanStore.subscribe(() => this.onStoreUpdate());
  }

  getElement(): HTMLElement {
    return this.container;
  }

  public getLayoutItems(): TreemapLayoutItem[] {
    return this.currentItems;
  }

  public setSize(width: number, height: number) {
    this.width = width;
    this.height = height;
    this.canvas.width = width;
    this.canvas.height = height;
  }

  private setupEvents() {
    this.breadcrumbBar.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest("[data-crumb-id]") as HTMLElement | null;
      if (!btn) return;
      const crumbId = parseInt(btn.dataset.crumbId || "", 10);
      if (!isNaN(crumbId)) {
        scanStore.zoomToBreadcrumb(crumbId);
      }
    });

    this.canvas.addEventListener("mousemove", (e) => this.handleMouseMove(e));
    this.canvas.addEventListener("mouseleave", () => this.handleMouseLeave());
    this.canvas.addEventListener("click", (e) => this.handleClick(e));
    this.canvas.addEventListener("dblclick", (e) => this.handleDoubleClick(e));
  }

  private setupResizeObserver() {
    if (typeof ResizeObserver === "undefined") return;
    this.resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        if (width > 0 && height > 0) {
          this.width = Math.floor(width);
          this.height = Math.floor(height);
          this.canvas.width = this.width;
          this.canvas.height = this.height;
          this.scheduleRender();
        }
      }
    });
    this.resizeObserver.observe(this.canvasWrapper);
  }

  private findItemAt(x: number, y: number): TreemapLayoutItem | null {
    for (let i = this.currentItems.length - 1; i >= 0; i--) {
      const item = this.currentItems[i];
      const r = item.rect;
      if (x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h) {
        return item;
      }
    }
    return null;
  }

  private handleMouseMove(e: MouseEvent) {
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const item = this.findItemAt(x, y);
    if (!item) {
      this.handleMouseLeave();
      return;
    }

    scanStore.hoverNode(item.nodeId);

    // Position and populate tooltip
    const node = scanStore.nodeMap.get(item.nodeId);
    const parentNode = node ? scanStore.nodeMap.get(node.parent_id) : null;
    const parentBytes = parentNode ? parentNode.total_bytes : item.totalBytes;
    const pct = parentBytes > 0 ? (item.totalBytes / parentBytes) * 100 : 100;

    const color = scanStore.palette.getColor(item.extension);

    this.tooltip.innerHTML = `
      <div class="flex items-center space-x-1.5 font-medium text-text-primary mb-1">
        <span class="w-2.5 h-2.5 rounded-sm inline-block" style="background-color: ${color}"></span>
        <span class="truncate font-mono">${item.name}</span>
      </div>
      <div class="text-slate-300 tabular-nums">
        <span>Size: <strong class="text-text-primary">${formatBytes(item.totalBytes)}</strong></span>
        <span class="ml-2 text-text-muted">(${pct.toFixed(1)}% of dir)</span>
      </div>
      <div class="text-[10px] text-text-muted font-mono truncate mt-0.5">
        ${item.isDir ? "Directory (double click to zoom)" : item.extension || "File"}
      </div>
    `;

    this.tooltip.style.display = "block";
    const tipWidth = 240;
    const tipHeight = 60;
    const posX = Math.min(x + 12, this.width - tipWidth);
    const posY = Math.min(y + 12, this.height - tipHeight);
    this.tooltip.style.left = `${Math.max(4, posX)}px`;
    this.tooltip.style.top = `${Math.max(4, posY)}px`;
  }

  private handleMouseLeave() {
    scanStore.hoverNode(null);
    this.tooltip.style.display = "none";
  }

  private handleClick(e: MouseEvent) {
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const item = this.findItemAt(x, y);
    if (item) {
      scanStore.selectNode(item.nodeId);
    }
  }

  private handleDoubleClick(e: MouseEvent) {
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const item = this.findItemAt(x, y);
    if (item && item.isDir) {
      scanStore.drillDown(item.nodeId);
    }
  }

  private onStoreUpdate() {
    this.renderBreadcrumbs();
    this.scheduleRender();
  }

  private renderBreadcrumbs() {
    const breadcrumb = scanStore.breadcrumb;
    let html = `<div class="flex items-center space-x-1 overflow-x-auto">`;

    for (let i = 0; i < breadcrumb.length; i++) {
      const id = breadcrumb[i];
      const node = scanStore.nodeMap.get(id);
      const name = node ? (node.name || "/") : "root";
      const isLast = i === breadcrumb.length - 1;

      html += `
        <button data-crumb-id="${id}" class="px-1.5 py-0.5 rounded font-mono text-[11px] transition-colors ${
          isLast
            ? "bg-accent-blue/20 text-accent-cyan font-medium pointer-events-none"
            : "hover:bg-surface-hover text-slate-400 hover:text-text-primary"
        }">
          ${name}
        </button>
      `;

      if (!isLast) {
        html += `<span class="text-surface-border">/</span>`;
      }
    }

    html += `</div>`;

    if (breadcrumb.length > 1) {
      html += `
        <button data-crumb-id="0" class="text-[11px] text-accent-cyan hover:underline flex items-center flex-shrink-0 ml-2">
          Reset Zoom
        </button>
      `;
    }

    this.breadcrumbBar.innerHTML = html;
  }

  public scheduleRender() {
    if (this.animFrameId !== null) return;
    if (typeof requestAnimationFrame === "undefined") {
      this.render();
      return;
    }
    this.animFrameId = requestAnimationFrame(() => {
      this.animFrameId = null;
      this.render();
    });
  }

  public render() {
    if (!this.ctx || this.width <= 0 || this.height <= 0) return;

    const bounds: Rect = { x: 0, y: 0, w: this.width, h: this.height };
    const rootId = scanStore.visualRootId;
    const nodes = scanStore.treeNodes;

    if (nodes.length === 0) {
      this.ctx.fillStyle = "#0F172A";
      this.ctx.fillRect(0, 0, this.width, this.height);
      this.currentItems = [];
      return;
    }

    // 1. Compute squarified layout with cushion hierarchy
    this.currentItems = buildTreemapLayout(nodes, rootId, bounds, 5);

    // 2. Build color map from palette manager
    const colorMap = new Map<string, string>();
    for (const stat of scanStore.extensionStats) {
      colorMap.set(stat.extension, scanStore.palette.getColor(stat.extension));
    }

    // 3. Render illuminated cushion treemap onto Canvas2D
    renderCushionTreemap(
      this.ctx,
      this.currentItems,
      colorMap,
      this.width,
      this.height,
      scanStore.hoveredNodeId,
      scanStore.selectedNodeId,
      scanStore.isolatedExtension
    );
  }
}
