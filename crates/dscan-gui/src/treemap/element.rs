use crate::theme;
use crate::treemap::squarify::LaidOutTreemapNode;
use gpui::Refineable as _;
use gpui::{
    App, BorderStyle, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    Pixels, Style, StyleRefinement, Styled, Window, fill, outline, point, px, size,
};

pub struct CushionTreemapElement {
    nodes: Vec<LaidOutTreemapNode>,
    selected_id: Option<u32>,
    hovered_id: Option<u32>,
    filter_ext: Option<String>,
    bg_color: gpui::Rgba,
    select_color: gpui::Rgba,
    hover_color: gpui::Rgba,
    style: StyleRefinement,
}

impl CushionTreemapElement {
    pub fn new(nodes: Vec<LaidOutTreemapNode>) -> Self {
        Self {
            nodes,
            selected_id: None,
            hovered_id: None,
            filter_ext: None,
            bg_color: theme::BG_DARK,
            select_color: theme::ACCENT_BLUE,
            hover_color: theme::TEXT_PRIMARY,
            style: StyleRefinement::default(),
        }
    }

    pub fn bg_color(mut self, color: gpui::Rgba) -> Self {
        self.bg_color = color;
        self
    }

    pub fn select_color(mut self, color: gpui::Rgba) -> Self {
        self.select_color = color;
        self
    }

    pub fn hover_color(mut self, color: gpui::Rgba) -> Self {
        self.hover_color = color;
        self
    }

    pub fn selected_id(mut self, id: Option<u32>) -> Self {
        self.selected_id = id;
        self
    }

    pub fn hovered_id(mut self, id: Option<u32>) -> Self {
        self.hovered_id = id;
        self
    }

    pub fn filter_ext(mut self, ext: Option<String>) -> Self {
        self.filter_ext = ext;
        self
    }
}

impl IntoElement for CushionTreemapElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for CushionTreemapElement {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Element for CushionTreemapElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        window.paint_quad(fill(bounds, self.bg_color));

        let origin_x = bounds.origin.x;
        let origin_y = bounds.origin.y;

        for node in &self.nodes {
            if node.is_dir {
                continue;
            }

            let r = node.rect;
            if r.w < 0.5 || r.h < 0.5 {
                continue;
            }

            let quad_bounds = Bounds {
                origin: point(origin_x + px(r.x), origin_y + px(r.y)),
                size: size(px(r.w), px(r.h)),
            };

            let base_color = theme::extension_color(&node.extension);
            let mut shaded_color =
                node.cushion
                    .shade_color(base_color, r.x, r.x + r.w, r.y, r.y + r.h);

            if let Some(ref filter) = self.filter_ext
                && !node.extension.eq_ignore_ascii_case(filter)
            {
                shaded_color.r *= 0.2;
                shaded_color.g *= 0.2;
                shaded_color.b *= 0.2;
            }

            window.paint_quad(fill(quad_bounds, shaded_color));

            if self.selected_id == Some(node.id) {
                window.paint_quad(outline(quad_bounds, self.select_color, BorderStyle::Solid));
            } else if self.hovered_id == Some(node.id) {
                window.paint_quad(outline(quad_bounds, self.hover_color, BorderStyle::Solid));
            }
        }
    }
}
