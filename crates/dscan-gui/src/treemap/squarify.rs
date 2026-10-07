use super::cushion::CushionSurface;
use dscan_core::TreemapNodeDto;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn area(&self) -> f32 {
        (self.w * self.h).max(0.0)
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }

    pub fn inset(&self, amount: f32) -> Self {
        let double = amount * 2.0;
        if self.w <= double || self.h <= double {
            *self
        } else {
            Self {
                x: self.x + amount,
                y: self.y + amount,
                w: self.w - double,
                h: self.h - double,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaidOutTreemapNode {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub extension: String,
    pub total_bytes: u64,
    pub is_dir: bool,
    pub has_children: bool,
    pub depth: u16,
    pub rect: Rect,
    pub cushion: CushionSurface,
}

#[derive(Debug, Clone, Copy)]
struct SquarifyItem {
    id: u32,
    weight: f32,
}

/// Squarify partition algorithm: arranges items with areas into approximately square boxes
pub fn squarify_partition(items: &[(u32, f32)], container: Rect) -> Vec<(u32, Rect)> {
    if items.is_empty() || container.w <= 0.0 || container.h <= 0.0 {
        return Vec::new();
    }

    let total_weight: f32 = items.iter().map(|(_, w)| *w).sum();
    if total_weight <= 0.0 {
        return Vec::new();
    }

    let container_area = container.area();
    let mut normalized_items: Vec<SquarifyItem> = items
        .iter()
        .filter(|(_, w)| *w > 0.0)
        .map(|(id, w)| SquarifyItem {
            id: *id,
            weight: (*w / total_weight) * container_area,
        })
        .collect();

    if normalized_items.is_empty() {
        return Vec::new();
    }

    // Sort descending by weight
    normalized_items.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut results = Vec::with_capacity(normalized_items.len());
    let mut remaining_box = container;
    let mut current_row: Vec<SquarifyItem> = Vec::new();

    for item in normalized_items {
        if current_row.is_empty() {
            current_row.push(item);
            continue;
        }

        let short_side = remaining_box.w.min(remaining_box.h);
        if short_side <= 0.001 {
            break;
        }

        let current_worst = worst_aspect_ratio(&current_row, short_side);
        let mut test_row = current_row.clone();
        test_row.push(item);
        let test_worst = worst_aspect_ratio(&test_row, short_side);

        if test_worst <= current_worst {
            current_row = test_row;
        } else {
            // Freeze current row and place its rectangles
            place_row(&current_row, &mut remaining_box, &mut results);
            current_row.clear();
            current_row.push(item);
        }
    }

    if !current_row.is_empty() {
        place_row(&current_row, &mut remaining_box, &mut results);
    }

    results
}

fn worst_aspect_ratio(row: &[SquarifyItem], side_length: f32) -> f32 {
    if row.is_empty() || side_length <= 0.0 {
        return f32::MAX;
    }

    let row_sum: f32 = row.iter().map(|it| it.weight).sum();
    if row_sum <= 0.0 {
        return f32::MAX;
    }

    let side_sq = side_length * side_length;
    let sum_sq = row_sum * row_sum;

    let mut worst = 0.0f32;
    for it in row {
        let area = it.weight.max(0.0001);
        let r1 = (side_sq * area) / sum_sq;
        let r2 = sum_sq / (side_sq * area);
        let aspect = r1.max(r2);
        if aspect > worst {
            worst = aspect;
        }
    }

    worst
}

fn place_row(row: &[SquarifyItem], bounds: &mut Rect, out: &mut Vec<(u32, Rect)>) {
    let row_sum: f32 = row.iter().map(|it| it.weight).sum();
    if row_sum <= 0.0 || bounds.w <= 0.0 || bounds.h <= 0.0 {
        return;
    }

    let is_horizontal = bounds.w >= bounds.h;
    if is_horizontal {
        // Vertical strip along left edge
        let strip_width = (row_sum / bounds.h).min(bounds.w);
        let mut curr_y = bounds.y;

        for it in row {
            let item_h = if row_sum > 0.0 {
                (it.weight / row_sum) * bounds.h
            } else {
                0.0
            };
            out.push((
                it.id,
                Rect {
                    x: bounds.x,
                    y: curr_y,
                    w: strip_width,
                    h: item_h,
                },
            ));
            curr_y += item_h;
        }

        bounds.x += strip_width;
        bounds.w = (bounds.w - strip_width).max(0.0);
    } else {
        // Horizontal strip along top edge
        let strip_height = (row_sum / bounds.w).min(bounds.h);
        let mut curr_x = bounds.x;

        for it in row {
            let item_w = if row_sum > 0.0 {
                (it.weight / row_sum) * bounds.w
            } else {
                0.0
            };
            out.push((
                it.id,
                Rect {
                    x: curr_x,
                    y: bounds.y,
                    w: item_w,
                    h: strip_height,
                },
            ));
            curr_x += item_w;
        }

        bounds.y += strip_height;
        bounds.h = (bounds.h - strip_height).max(0.0);
    }
}

/// Recursively build layout and cushion parameters from DTO node hierarchy
pub fn build_hierarchical_layout(
    nodes: &[TreemapNodeDto],
    root_bounds: Rect,
) -> Vec<LaidOutTreemapNode> {
    if nodes.is_empty() || root_bounds.w <= 1.0 || root_bounds.h <= 1.0 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(nodes.len());
    let mut initial_cushion = CushionSurface::new();
    initial_cushion.add_ridge(
        root_bounds.x,
        root_bounds.x + root_bounds.w,
        root_bounds.y,
        root_bounds.y + root_bounds.h,
        0.5,
    );

    let root_node = &nodes[0];
    result.push(LaidOutTreemapNode {
        id: root_node.id,
        parent_id: root_node.parent_id,
        name: root_node.name.clone(),
        extension: root_node.extension.clone(),
        total_bytes: root_node.total_bytes,
        is_dir: root_node.is_dir,
        has_children: root_node.is_dir && !root_node.children_ids.is_empty(),
        depth: root_node.rel_depth,
        rect: root_bounds,
        cushion: initial_cushion,
    });

    // Work queue: (node_index, rect, cushion)
    let mut queue = vec![(0usize, root_bounds, initial_cushion)];

    while let Some((node_idx, parent_rect, parent_cushion)) = queue.pop() {
        let node = &nodes[node_idx];
        if node.children_ids.is_empty() {
            continue;
        }

        // Gather children weights
        let children_items: Vec<(u32, f32)> = node
            .children_ids
            .iter()
            .filter_map(|&cid| {
                nodes
                    .get(cid as usize)
                    .map(|child| (cid, child.total_bytes.max(1) as f32))
            })
            .collect();

        // 2px inset for directory nesting clarity
        let layout_box = if node.rel_depth > 0 {
            parent_rect.inset(2.0)
        } else {
            parent_rect
        };

        let placed_children = squarify_partition(&children_items, layout_box);

        for (child_id, child_rect) in placed_children {
            let child_idx = child_id as usize;
            if child_idx >= nodes.len() {
                continue;
            }
            let child_node = &nodes[child_idx];

            let mut child_cushion = parent_cushion;
            let depth = child_node.rel_depth;
            let decay = 0.55f32.powi(depth as i32);
            let h = (0.6 * decay).max(0.02);

            child_cushion.add_ridge(
                child_rect.x,
                child_rect.x + child_rect.w,
                child_rect.y,
                child_rect.y + child_rect.h,
                h,
            );

            let has_children = child_node.is_dir && !child_node.children_ids.is_empty();

            result.push(LaidOutTreemapNode {
                id: child_node.id,
                parent_id: child_node.parent_id,
                name: child_node.name.clone(),
                extension: child_node.extension.clone(),
                total_bytes: child_node.total_bytes,
                is_dir: child_node.is_dir,
                has_children,
                depth,
                rect: child_rect,
                cushion: child_cushion,
            });

            if has_children && child_rect.w > 4.0 && child_rect.h > 4.0 {
                queue.push((child_idx, child_rect, child_cushion));
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_squarify_partition_basic() {
        let items = vec![
            (1, 6.0),
            (2, 6.0),
            (3, 4.0),
            (4, 3.0),
            (5, 2.0),
            (6, 2.0),
            (7, 1.0),
        ];
        let container = Rect::new(0.0, 0.0, 600.0, 400.0);
        let placed = squarify_partition(&items, container);

        assert_eq!(placed.len(), items.len());
        let total_area: f32 = placed.iter().map(|(_, r)| r.area()).sum();
        let expected_area = container.area();
        assert!((total_area - expected_area).abs() < 1.0);

        for (_, r) in &placed {
            assert!(container.contains(r.x, r.y));
            assert!(r.w > 0.0 && r.h > 0.0);
        }
    }

    #[test]
    fn test_squarify_empty_and_zero() {
        let empty = squarify_partition(&[], Rect::new(0.0, 0.0, 100.0, 100.0));
        assert!(empty.is_empty());

        let zero_box = squarify_partition(&[(1, 10.0)], Rect::new(0.0, 0.0, 0.0, 100.0));
        assert!(zero_box.is_empty());
    }

    #[test]
    fn test_build_hierarchical_layout() {
        let nodes = vec![
            TreemapNodeDto {
                id: 0,
                parent_id: 0,
                name: "root".to_string(),
                total_bytes: 1000,
                direct_bytes: 0,
                rel_depth: 0,
                is_dir: true,
                extension: String::new(),
                children_ids: vec![1, 2],
            },
            TreemapNodeDto {
                id: 1,
                parent_id: 0,
                name: "sub".to_string(),
                total_bytes: 600,
                direct_bytes: 0,
                rel_depth: 1,
                is_dir: true,
                extension: String::new(),
                children_ids: vec![3],
            },
            TreemapNodeDto {
                id: 2,
                parent_id: 0,
                name: "file.mp4".to_string(),
                total_bytes: 400,
                direct_bytes: 400,
                rel_depth: 1,
                is_dir: false,
                extension: ".mp4".to_string(),
                children_ids: vec![],
            },
            TreemapNodeDto {
                id: 3,
                parent_id: 1,
                name: "main.rs".to_string(),
                total_bytes: 600,
                direct_bytes: 600,
                rel_depth: 2,
                is_dir: false,
                extension: ".rs".to_string(),
                children_ids: vec![],
            },
        ];

        let bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        let layout = build_hierarchical_layout(&nodes, bounds);
        assert!(!layout.is_empty());
        assert_eq!(layout[0].id, 0);

        let rs_node = layout.iter().find(|n| n.name == "main.rs");
        assert!(rs_node.is_some());
        let rs = rs_node.unwrap();
        assert_eq!(rs.extension, ".rs");
        assert!(rs.rect.w > 0.0 && rs.rect.h > 0.0);
    }
}
