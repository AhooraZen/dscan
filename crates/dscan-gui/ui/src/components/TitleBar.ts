/**
 * Top Toolbar / TitleBar Component.
 * Contains drive selector, path input, Scan / Pause / Cancel buttons, and config toggles.
 */

import { scanController } from "../state/scanController";
import { scanStore } from "../state/scanStore";

export class TitleBar {
  private container: HTMLElement;
  private pathInput!: HTMLInputElement;
  private driveSelect!: HTMLSelectElement;
  private scanBtn!: HTMLButtonElement;
  private pauseBtn!: HTMLButtonElement;
  private cancelBtn!: HTMLButtonElement;

  constructor() {
    this.container = document.createElement("div");
    this.container.className =
      "h-11 w-full bg-surface-card border-b border-surface-border px-3 flex items-center justify-between text-xs select-none gap-2";

    this.buildUI();
    this.setupEvents();
    this.updateUIState();
    scanStore.subscribe(() => this.updateUIState());
  }

  getElement(): HTMLElement {
    return this.container;
  }

  private buildUI() {
    this.container.innerHTML = `
      <div class="flex items-center space-x-2">
        <div class="flex items-center space-x-1.5 font-bold tracking-tight text-text-primary text-sm mr-2">
          <svg class="w-5 h-5 text-accent-cyan" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
            <rect width="20" height="8" x="2" y="2" rx="2" />
            <rect width="20" height="8" x="2" y="14" rx="2" />
            <line x1="6" x2="6.01" y1="6" y2="6" />
            <line x1="6" x2="6.01" y1="18" y2="18" />
          </svg>
          <span class="bg-gradient-to-r from-accent-cyan to-accent-blue bg-clip-text text-transparent">dscan</span>
        </div>

        <select data-drive-select class="bg-surface-panel border border-surface-border rounded px-2 py-1 text-xs text-text-primary outline-none focus:border-accent-cyan cursor-pointer">
          <option value="/">Root (/)</option>
        </select>
      </div>

      <div class="flex-1 max-w-xl flex items-center space-x-1">
        <input
          data-path-input
          type="text"
          value="/"
          placeholder="Enter path to analyze..."
          class="flex-1 bg-surface-panel border border-surface-border rounded px-2.5 py-1 text-xs text-text-primary font-mono outline-none focus:border-accent-cyan focus:ring-1 focus:ring-accent-cyan/30"
        />
      </div>

      <div class="flex items-center space-x-1.5">
        <button
          data-scan-btn
          class="flex items-center space-x-1 bg-accent-blue hover:bg-accent-blue/90 text-white font-medium px-3 py-1 rounded transition-colors shadow-sm"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/></svg>
          <span>Scan</span>
        </button>

        <button
          data-pause-btn
          disabled
          class="flex items-center space-x-1 bg-surface-panel hover:bg-surface-hover text-text-muted hover:text-text-primary border border-surface-border px-2.5 py-1 rounded transition-colors disabled:opacity-40 disabled:pointer-events-none"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
          <span data-pause-label>Pause</span>
        </button>

        <button
          data-cancel-btn
          disabled
          class="flex items-center space-x-1 bg-surface-panel hover:bg-surface-hover text-accent-red border border-surface-border px-2.5 py-1 rounded transition-colors disabled:opacity-40 disabled:pointer-events-none"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect width="18" height="18" x="3" y="3" rx="2"/><line x1="9" y1="9" x2="15" y2="15"/><line x1="15" y1="9" x2="9" y2="15"/></svg>
          <span>Cancel</span>
        </button>
      </div>
    `;

    this.driveSelect = this.container.querySelector("[data-drive-select]") as HTMLSelectElement;
    this.pathInput = this.container.querySelector("[data-path-input]") as HTMLInputElement;
    this.scanBtn = this.container.querySelector("[data-scan-btn]") as HTMLButtonElement;
    this.pauseBtn = this.container.querySelector("[data-pause-btn]") as HTMLButtonElement;
    this.cancelBtn = this.container.querySelector("[data-cancel-btn]") as HTMLButtonElement;
  }

  private setupEvents() {
    this.driveSelect.addEventListener("change", () => {
      this.pathInput.value = this.driveSelect.value;
    });

    this.pathInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        this.triggerScan();
      }
    });

    this.scanBtn.addEventListener("click", () => this.triggerScan());
    this.pauseBtn.addEventListener("click", () => scanController.togglePause());
    this.cancelBtn.addEventListener("click", () => scanController.cancelScan());
  }

  private triggerScan() {
    const path = this.pathInput.value.trim() || "/";
    scanController.startScan(path);
  }

  public setDrives(drives: { path: string; name: string }[]) {
    let html = "";
    for (const d of drives) {
      html += `<option value="${d.path}">${d.name} (${d.path})</option>`;
    }
    this.driveSelect.innerHTML = html;
    if (drives.length > 0) {
      this.pathInput.value = drives[0].path;
    }
  }

  public updateUIState() {
    const isScanning = scanStore.isScanning;
    const isPaused = scanStore.isPaused;

    this.scanBtn.disabled = isScanning;
    this.pauseBtn.disabled = !isScanning;
    this.cancelBtn.disabled = !isScanning;

    const pauseLabel = this.container.querySelector("[data-pause-label]");
    if (pauseLabel) {
      pauseLabel.textContent = isPaused ? "Resume" : "Pause";
    }
  }
}
