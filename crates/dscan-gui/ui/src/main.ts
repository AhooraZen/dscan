/**
 * dscan-gui Main Application Entrypoint.
 * Assembles WinDirStat 3-pane desktop visualization and connects to Tauri v2 backend.
 */

import "./styles/main.css";
import { Layout } from "./components/Layout";
import { StatusBar } from "./components/StatusBar";
import { TitleBar } from "./components/TitleBar";
import { scanController } from "./state/scanController";
import { scanStore } from "./state/scanStore";

function initApp() {
  const appRoot = document.getElementById("app");
  if (!appRoot) {
    throw new Error("Target #app root container not found");
  }

  // 1. Top Title & Command Bar
  const titleBar = new TitleBar();
  appRoot.appendChild(titleBar.getElement());

  // 2. WinDirStat 3-Pane Resizable Layout
  const layout = new Layout();
  appRoot.appendChild(layout.getElement());

  // 3. Bottom Status Bar
  const statusBar = new StatusBar();
  appRoot.appendChild(statusBar.getElement());

  // 4. Initial Drive Discovery
  scanStore.subscribe(() => {
    if (scanStore.drives.length > 0) {
      titleBar.setDrives(scanStore.drives);
    }
  });

  scanController.loadDrives();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initApp);
} else {
  initApp();
}
