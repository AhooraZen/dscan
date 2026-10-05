/**
 * Classic WinDirStat Pacman Chew Animation.
 * Renders an animated yellow Pacman eating moving dots on a compact canvas.
 */

export class PacmanAnimator {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D | null;
  private animId: number | null = null;
  private startTime: number = 0;

  constructor(width: number = 32, height: number = 18) {
    this.canvas = typeof document !== "undefined" ? document.createElement("canvas") : ({} as HTMLCanvasElement);
    this.canvas.width = width;
    this.canvas.height = height;
    this.canvas.className = "inline-block align-middle";
    this.ctx = this.canvas.getContext ? this.canvas.getContext("2d") : null;
    this.startTime = performance.now();
  }

  getElement(): HTMLCanvasElement {
    return this.canvas;
  }

  start() {
    if (this.animId !== null || !this.ctx) return;
    const loop = (now: number) => {
      this.renderFrame(now);
      this.animId = requestAnimationFrame(loop);
    };
    this.animId = requestAnimationFrame(loop);
  }

  stop() {
    if (this.animId !== null) {
      cancelAnimationFrame(this.animId);
      this.animId = null;
    }
    if (this.ctx) {
      this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    }
  }

  renderFrame(time: number) {
    const ctx = this.ctx;
    if (!ctx) return;
    const w = this.canvas.width;
    const h = this.canvas.height;
    ctx.clearRect(0, 0, w, h);

    const elapsed = (time - this.startTime) / 1000;
    // 10Hz mouth chew frequency
    const mouthCycle = Math.abs(Math.sin(elapsed * Math.PI * 10));
    const mouthAngle = (mouthCycle * 45 * Math.PI) / 180;

    const pacX = 10;
    const pacY = h / 2;
    const radius = Math.min(pacY - 1, 7);

    // Draw Pacman body (facing right)
    ctx.fillStyle = "#FACC15"; // Yellow 400
    ctx.beginPath();
    ctx.moveTo(pacX, pacY);
    ctx.arc(pacX, pacY, radius, mouthAngle / 2, 2 * Math.PI - mouthAngle / 2);
    ctx.closePath();
    ctx.fill();

    // Draw Eye
    ctx.fillStyle = "#0F172A";
    ctx.beginPath();
    ctx.arc(pacX + 1, pacY - radius * 0.45, 1.2, 0, 2 * Math.PI);
    ctx.fill();

    // Draw moving food dots
    ctx.fillStyle = "#94A3B8"; // Slate 400
    const dotSpacing = 8;
    const dotSpeed = 30; // pixels per second
    const dotOffset = (elapsed * dotSpeed) % dotSpacing;

    for (let x = w; x >= pacX + radius; x -= dotSpacing) {
      const curX = x - dotOffset;
      if (curX > pacX + radius && curX < w) {
        ctx.beginPath();
        ctx.arc(curX, pacY, 1.5, 0, 2 * Math.PI);
        ctx.fill();
      }
    }
  }
}
