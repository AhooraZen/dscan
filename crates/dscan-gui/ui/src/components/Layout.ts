/**
 * WinDirStat 3-Pane Resizable Layout Component.
 * - Top Left: Directory Tree
 * - Top Right: Extension Legend
 * - Bottom: Cushion Treemap
 * Features draggable vertical and horizontal splitters.
 */

import { CushionTreemap } from "./CushionTreemap";
import { DirectoryTree } from "./DirectoryTree";
import { ExtensionLegend } from "./ExtensionLegend";

export class Layout {
  private container: HTMLElement;
  private topRow: HTMLElement;
  private bottomRow: HTMLElement;
  private leftTopPane: HTMLElement;
  private rightTopPane: HTMLElement;

  private vSplitter: HTMLElement;
  private hSplitter: HTMLElement;

  private dirTree: DirectoryTree;
  private extLegend: ExtensionLegend;
  private cushionTreemap: CushionTreemap;

  private topHeightPercent: number = 48; // % of total height
  private leftWidthPercent: number = 62;  // % of top row width

  constructor() {
    this.container = document.createElement("div");
    this.container.className = "flex-1 flex flex-col w-full h-full overflow-hidden select-none";

    // 1. Top Row (Tree + Legend)
    this.topRow = document.createElement("div");
    this.topRow.className = "flex flex-row w-full overflow-hidden relative";
    this.topRow.style.height = `${this.topHeightPercent}%`;

    this.leftTopPane = document.createElement("div");
    this.leftTopPane.className = "h-full overflow-hidden";
    this.leftTopPane.style.width = `${this.leftWidthPercent}%`;

    this.vSplitter = document.createElement("div");
    this.vSplitter.className =
      "w-1.5 h-full bg-surface-border hover:bg-accent-cyan cursor-col-resize flex-shrink-0 transition-colors z-10";

    this.rightTopPane = document.createElement("div");
    this.rightTopPane.className = "h-full flex-1 overflow-hidden";

    this.dirTree = new DirectoryTree();
    this.leftTopPane.appendChild(this.dirTree.getElement());

    this.extLegend = new ExtensionLegend();
    this.rightTopPane.appendChild(this.extLegend.getElement());

    this.topRow.appendChild(this.leftTopPane);
    this.topRow.appendChild(this.vSplitter);
    this.topRow.appendChild(this.rightTopPane);

    // 2. Horizontal Splitter
    this.hSplitter = document.createElement("div");
    this.hSplitter.className =
      "h-1.5 w-full bg-surface-border hover:bg-accent-cyan cursor-row-resize flex-shrink-0 transition-colors z-10";

    // 3. Bottom Row (Cushion Treemap)
    this.bottomRow = document.createElement("div");
    this.bottomRow.className = "flex-1 w-full overflow-hidden relative";

    this.cushionTreemap = new CushionTreemap();
    this.bottomRow.appendChild(this.cushionTreemap.getElement());

    this.container.appendChild(this.topRow);
    this.container.appendChild(this.hSplitter);
    this.container.appendChild(this.bottomRow);

    this.setupSplitters();
  }

  getElement(): HTMLElement {
    return this.container;
  }

  public getDirectoryTree(): DirectoryTree {
    return this.dirTree;
  }

  public getExtensionLegend(): ExtensionLegend {
    return this.extLegend;
  }

  public getCushionTreemap(): CushionTreemap {
    return this.cushionTreemap;
  }

  private setupSplitters() {
    // Vertical Splitter (Left-Right in Top Row)
    let isDraggingV = false;
    this.vSplitter.addEventListener("mousedown", (e) => {
      e.preventDefault();
      isDraggingV = true;
      document.body.style.cursor = "col-resize";
    });

    // Horizontal Splitter (Top-Bottom)
    let isDraggingH = false;
    this.hSplitter.addEventListener("mousedown", (e) => {
      e.preventDefault();
      isDraggingH = true;
      document.body.style.cursor = "row-resize";
    });

    window.addEventListener("mousemove", (e) => {
      if (isDraggingV) {
        const topRect = this.topRow.getBoundingClientRect();
        const offsetX = e.clientX - topRect.left;
        const newPct = Math.min(85, Math.max(15, (offsetX / topRect.width) * 100));
        this.leftWidthPercent = newPct;
        this.leftTopPane.style.width = `${newPct}%`;
      } else if (isDraggingH) {
        const containerRect = this.container.getBoundingClientRect();
        const offsetY = e.clientY - containerRect.top;
        const newPct = Math.min(80, Math.max(20, (offsetY / containerRect.height) * 100));
        this.topHeightPercent = newPct;
        this.topRow.style.height = `${newPct}%`;
      }
    });

    window.addEventListener("mouseup", () => {
      if (isDraggingV || isDraggingH) {
        isDraggingV = false;
        isDraggingH = false;
        document.body.style.cursor = "";
        this.cushionTreemap.scheduleRender();
      }
    });
  }
}
