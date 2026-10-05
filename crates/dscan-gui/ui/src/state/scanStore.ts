import { PaletteManager } from "../styles/palette";
import type { TreemapLayoutItem, TreemapNode } from "../treemap/squarify";

export interface ScanProgress {
  totalBytes: number;
  totalFiles: number;
  activeWorkers: number;
  currentPath: string;
  elapsedMillis: number;
  filesPerSec: number;
  bytesPerSec: number;
  isComplete: boolean;
  isPaused: boolean;
}

export interface ExtensionStat {
  extension: string;
  totalBytes: number;
  fileCount: number;
  percentageOfTotal: number;
}

export interface DriveInfo {
  path: string;
  name: string;
  total_bytes: number;
  free_bytes: number;
}

class ScanStore {
  isScanning: boolean = false;
  isPaused: boolean = false;
  progress: ScanProgress = {
    totalBytes: 0,
    totalFiles: 0,
    activeWorkers: 0,
    currentPath: "",
    elapsedMillis: 0,
    filesPerSec: 0,
    bytesPerSec: 0,
    isComplete: false,
    isPaused: false,
  };

  rootPath: string = "";
  treeNodes: TreemapNode[] = [];
  nodeMap: Map<number, TreemapNode> = new Map();
  layoutItems: TreemapLayoutItem[] = [];

  selectedNodeId: number | null = null;
  hoveredNodeId: number | null = null;
  visualRootId: number = 0;
  breadcrumb: number[] = [0];

  extensionStats: ExtensionStat[] = [];
  palette = new PaletteManager();
  isolatedExtension: string | null = null;

  drives: DriveInfo[] = [];

  private listeners = new Set<() => void>();

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  notify() {
    for (const listener of this.listeners) {
      listener();
    }
  }

  setScanning(scanning: boolean) {
    this.isScanning = scanning;
    this.notify();
  }

  setPaused(paused: boolean) {
    this.isPaused = paused;
    this.notify();
  }

  updateProgress(p: Partial<ScanProgress>) {
    this.progress = { ...this.progress, ...p };
    this.notify();
  }

  setTreeData(nodes: TreemapNode[], rootPath: string) {
    this.treeNodes = nodes;
    this.rootPath = rootPath;
    this.nodeMap.clear();
    for (const n of nodes) {
      this.nodeMap.set(n.id, n);
    }
    this.visualRootId = 0;
    this.breadcrumb = [0];
    this.selectedNodeId = 0;
    this.notify();
  }

  setExtensions(stats: ExtensionStat[]) {
    this.extensionStats = stats;
    this.palette.reset();
    for (const s of stats) {
      this.palette.getColor(s.extension);
    }
    this.notify();
  }

  selectNode(id: number | null) {
    if (this.selectedNodeId !== id) {
      this.selectedNodeId = id;
      this.notify();
    }
  }

  hoverNode(id: number | null) {
    if (this.hoveredNodeId !== id) {
      this.hoveredNodeId = id;
      this.notify();
    }
  }

  drillDown(nodeId: number) {
    const node = this.nodeMap.get(nodeId);
    if (node && node.is_dir) {
      this.visualRootId = nodeId;
      if (!this.breadcrumb.includes(nodeId)) {
        this.breadcrumb.push(nodeId);
      }
      this.notify();
    }
  }

  zoomToBreadcrumb(nodeId: number) {
    const idx = this.breadcrumb.indexOf(nodeId);
    if (idx !== -1) {
      this.breadcrumb = this.breadcrumb.slice(0, idx + 1);
      this.visualRootId = nodeId;
      this.notify();
    }
  }

  toggleIsolateExtension(ext: string) {
    if (this.isolatedExtension === ext) {
      this.isolatedExtension = null;
    } else {
      this.isolatedExtension = ext;
    }
    this.notify();
  }
}

export const scanStore = new ScanStore();
