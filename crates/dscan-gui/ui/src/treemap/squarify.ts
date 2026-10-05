/**
 * Bruls, Huizing, van Wijk (2000) Squarified Treemap Layout Engine.
 * Produces aspect ratios approaching 1.0, avoiding thin unreadable needles.
 */

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface TreemapNode {
  id: number;
  parent_id: number;
  name: string;
  total_bytes: number;
  direct_bytes: number;
  rel_depth: number;
  is_dir: boolean;
  extension: string;
  children_ids: number[];
}

export interface CushionSurface {
  x1: number;
  x2: number;
  y1: number;
  y2: number;
  depth: number;
}

export interface TreemapLayoutItem {
  nodeId: number;
  rect: Rect;
  isDir: boolean;
  extension: string;
  name: string;
  totalBytes: number;
  cushions: CushionSurface[];
}

/**
 * Calculates worst aspect ratio for a candidate row of areas along length L
 */
function worstAspectRatio(row: number[], rowSum: number, length: number): number {
  if (row.length === 0 || length <= 0 || rowSum <= 0) return Infinity;
  let minArea = Infinity;
  let maxArea = -Infinity;
  for (let i = 0; i < row.length; i++) {
    const a = row[i];
    if (a < minArea) minArea = a;
    if (a > maxArea) maxArea = a;
  }
  const lengthSq = length * length;
  const sumSq = rowSum * rowSum;
  return Math.max((lengthSq * maxArea) / sumSq, sumSq / (lengthSq * minArea));
}

/**
 * Squarifies a list of nodes within a bounding rectangle
 */
export function squarifyChildren(
  items: { node: TreemapNode; value: number }[],
  bounds: Rect,
  ancestorCushions: CushionSurface[] = []
): TreemapLayoutItem[] {
  if (bounds.w <= 2 || bounds.h <= 2 || items.length === 0) {
    return [];
  }

  const validItems = items.filter(it => it.value > 0);
  if (validItems.length === 0) return [];

  // Sort descending by size
  validItems.sort((a, b) => b.value - a.value);

  const totalValue = validItems.reduce((acc, it) => acc + it.value, 0);
  const totalArea = bounds.w * bounds.h;
  const areaScale = totalArea / totalValue;

  const areas = validItems.map(it => it.value * areaScale);
  const result: TreemapLayoutItem[] = [];

  let curBounds = { ...bounds };
  let startIdx = 0;

  while (startIdx < validItems.length) {
    const isHorizontal = curBounds.w >= curBounds.h;
    const length = isHorizontal ? curBounds.h : curBounds.w;

    const row: number[] = [areas[startIdx]];
    let rowSum = areas[startIdx];
    let endIdx = startIdx + 1;

    while (endIdx < validItems.length) {
      const nextArea = areas[endIdx];
      const nextRowSum = rowSum + nextArea;
      const currentWorst = worstAspectRatio(row, rowSum, length);
      row.push(nextArea);
      const nextWorst = worstAspectRatio(row, nextRowSum, length);

      if (nextWorst <= currentWorst) {
        rowSum = nextRowSum;
        endIdx++;
      } else {
        row.pop();
        break;
      }
    }

    // Lay out row
    const rowThickness = rowSum / length;
    let offset = 0;

    for (let i = startIdx; i < endIdx; i++) {
      const itemArea = areas[i];
      const itemLength = itemArea / rowThickness;
      const node = validItems[i].node;

      let itemRect: Rect;
      if (isHorizontal) {
        itemRect = {
          x: curBounds.x,
          y: curBounds.y + offset,
          w: rowThickness,
          h: itemLength,
        };
      } else {
        itemRect = {
          x: curBounds.x + offset,
          y: curBounds.y,
          w: itemLength,
          h: rowThickness,
        };
      }
      offset += itemLength;

      const currentCushion: CushionSurface = {
        x1: itemRect.x,
        x2: itemRect.x + itemRect.w,
        y1: itemRect.y,
        y2: itemRect.y + itemRect.h,
        depth: node.rel_depth,
      };

      const itemCushions = [...ancestorCushions, currentCushion];

      result.push({
        nodeId: node.id,
        rect: itemRect,
        isDir: node.is_dir,
        extension: node.extension,
        name: node.name,
        totalBytes: node.total_bytes,
        cushions: itemCushions,
      });
    }

    // Update remaining bounds
    if (isHorizontal) {
      curBounds.x += rowThickness;
      curBounds.w -= rowThickness;
    } else {
      curBounds.y += rowThickness;
      curBounds.h -= rowThickness;
    }

    startIdx = endIdx;
  }

  return result;
}

/**
 * Builds the complete multi-level squarified treemap layout from hierarchical nodes
 */
export function buildTreemapLayout(
  nodes: TreemapNode[],
  rootId: number,
  bounds: Rect,
  maxDisplayDepth: number = 6
): TreemapLayoutItem[] {
  if (nodes.length === 0) return [];

  const nodeMap = new Map<number, TreemapNode>();
  for (const n of nodes) {
    nodeMap.set(n.id, n);
  }

  const root = nodeMap.get(rootId);
  if (!root) return [];

  const layoutLeaves: TreemapLayoutItem[] = [];

  function recurse(
    currNode: TreemapNode,
    currBounds: Rect,
    cushions: CushionSurface[],
    currentDepth: number
  ) {
    if (currBounds.w <= 2 || currBounds.h <= 2) return;

    if (
      !currNode.is_dir ||
      currNode.children_ids.length === 0 ||
      currentDepth >= maxDisplayDepth
    ) {
      layoutLeaves.push({
        nodeId: currNode.id,
        rect: currBounds,
        isDir: currNode.is_dir,
        extension: currNode.extension,
        name: currNode.name,
        totalBytes: currNode.total_bytes,
        cushions,
      });
      return;
    }

    // Prepare children for squarify
    const childItems: { node: TreemapNode; value: number }[] = [];
    for (const cid of currNode.children_ids) {
      const child = nodeMap.get(cid);
      if (child && child.total_bytes > 0) {
        childItems.push({ node: child, value: child.total_bytes });
      }
    }

    if (childItems.length === 0) {
      layoutLeaves.push({
        nodeId: currNode.id,
        rect: currBounds,
        isDir: currNode.is_dir,
        extension: currNode.extension,
        name: currNode.name,
        totalBytes: currNode.total_bytes,
        cushions,
      });
      return;
    }

    const currentCushion: CushionSurface = {
      x1: currBounds.x,
      x2: currBounds.x + currBounds.w,
      y1: currBounds.y,
      y2: currBounds.y + currBounds.h,
      depth: currNode.rel_depth,
    };
    const nextCushions = [...cushions, currentCushion];

    const childLayouts = squarifyChildren(childItems, currBounds, nextCushions);
    for (const cl of childLayouts) {
      const childNode = nodeMap.get(cl.nodeId);
      if (childNode) {
        recurse(childNode, cl.rect, cl.cushions, currentDepth + 1);
      }
    }
  }

  recurse(root, bounds, [], 0);
  return layoutLeaves;
}
