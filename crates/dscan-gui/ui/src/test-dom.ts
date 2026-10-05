export class MockElement {
  tagName: string;
  className: string = "";
  style: Record<string, string> = {};
  children: MockElement[] = [];
  innerHTML: string = "";
  dataset: Record<string, string> = {};
  scrollTop: number = 0;
  clientHeight: number = 600;
  tabIndex: number = 0;
  width: number = 800;
  height: number = 600;

  constructor(tagName: string) {
    this.tagName = tagName;
  }

  appendChild(child: MockElement) {
    this.children.push(child);
    return child;
  }

  addEventListener() {}
  querySelector() { return null; }
  closest() { return null; }
  contains() { return false; }
  getBoundingClientRect() {
    return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
  }

  getContext() {
    return {
      clearRect: () => {},
      beginPath: () => {},
      arc: () => {},
      fill: () => {},
      moveTo: () => {},
      closePath: () => {},
      fillStyle: "",
      fillRect: () => {},
      createImageData: (w: number, h: number) => ({
        data: new Uint8ClampedArray(w * h * 4),
      }),
      putImageData: () => {},
      strokeRect: () => {},
    };
  }
}

const g = globalThis as Record<string, unknown>;
g.document = {
  createElement: (tag: string) => new MockElement(tag),
};
g.window = {
  addEventListener: () => {},
};
