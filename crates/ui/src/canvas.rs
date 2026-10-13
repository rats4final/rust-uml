//! The diagram canvas: draws a [`render::Scene`] and turns mouse input into
//! engine commands. It never edits the document itself.

use egui::{
    Color32, CornerRadius, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Vec2,
    text::{LayoutJob, TextFormat},
};
use render::{HitShape, Prim, Scene};
use serde_json::{Value, json};
use uml::{EdgeSpec, NodeSpec};

use crate::fonts;

const SELECT: Color32 = Color32::from_rgb(0x1e, 0x88, 0xe5);
const HANDLE: f32 = 7.0;
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    Node(&'static NodeSpec),
    Edge(&'static EdgeSpec),
}

#[derive(Debug, Clone, PartialEq)]
enum Drag {
    None,
    Pan,
    Move {
        views: Vec<String>,
        from: Pos2,
        to: Pos2,
    },
    Resize {
        view: String,
        rect: render::Rect,
        to: Pos2,
    },
    Edge {
        tail: String,
        from: Pos2,
        to: Pos2,
    },
}

pub struct Canvas {
    /// Screen offset of diagram point (0, 0), relative to the canvas corner.
    pan: Vec2,
    zoom: f32,
    drag: Drag,
    /// Where the canvas was drawn last frame.
    screen: Rect,
    /// Fit the diagram into view on the next frame (after a file or
    /// diagram opens, when the canvas size is known).
    fit_pending: bool,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            pan: Vec2::new(20.0, 20.0),
            zoom: 1.0,
            drag: Drag::None,
            screen: Rect::NOTHING,
            fit_pending: true,
        }
    }
}

/// What the canvas asks the app to do this frame.
#[derive(Default)]
pub struct Output {
    pub commands: Vec<(&'static str, Value)>,
    /// The user finished using a one-shot tool.
    pub tool_done: bool,
    /// Double-clicked a box: start editing its name.
    pub rename: bool,
}

impl Canvas {
    pub fn reset_view(&mut self) {
        *self = Self::default();
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// 100% zoom, diagram origin at the top-left.
    pub fn actual_size(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::new(20.0, 20.0);
        self.fit_pending = false;
    }

    /// Fit the whole diagram into view on the next frame.
    pub fn request_fit(&mut self) {
        self.fit_pending = true;
    }

    /// Zoom and pan so `bounds` fills `area` with a margin, never zooming in past 100%.
    fn fit(&mut self, bounds: render::Rect, area: Rect) {
        const MARGIN: f32 = 24.0;
        if bounds.w <= 0.0
            || bounds.h <= 0.0
            || area.width() <= 2.0 * MARGIN
            || area.height() <= 2.0 * MARGIN
        {
            self.pan = Vec2::new(20.0, 20.0) - Vec2::new(bounds.x as f32, bounds.y as f32);
            return;
        }
        let zx = (area.width() - 2.0 * MARGIN) / bounds.w as f32;
        let zy = (area.height() - 2.0 * MARGIN) / bounds.h as f32;
        self.zoom = zx.min(zy).clamp(MIN_ZOOM, 1.0);
        self.pan = Vec2::splat(MARGIN) - Vec2::new(bounds.x as f32, bounds.y as f32) * self.zoom;
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// Screen position of a diagram point, as of the last frame.
    #[cfg(test)]
    pub fn to_screen(&self, p: render::Point) -> Pos2 {
        self.screen.min + self.pan + Vec2::new(p.x as f32, p.y as f32) * self.zoom
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        scene: &Scene,
        selection: &[String],
        tool: Tool,
    ) -> Output {
        let mut out = Output::default();
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        self.screen = response.rect;
        if std::mem::take(&mut self.fit_pending) {
            self.fit(scene.bounds, response.rect);
        }
        let origin = response.rect.min + self.pan;
        let zoom = self.zoom;
        let to_screen =
            |x: f64, y: f64| Pos2::new(origin.x + x as f32 * zoom, origin.y + y as f32 * zoom);
        let to_diagram = |p: Pos2| ((p - origin) / zoom).to_pos2();

        painter.rect_filled(response.rect, CornerRadius::ZERO, Color32::from_gray(250));
        self.paint_scene(&painter, scene, &to_screen);
        self.paint_selection(&painter, scene, selection, &to_screen, tool);

        // Zoom with Ctrl+wheel / pinch around the pointer; pan with the wheel.
        if let Some(hover) = response.hover_pos() {
            let (scroll, zoom_delta) = ui.input(|i| (i.smooth_scroll_delta(), i.zoom_delta()));
            if zoom_delta != 1.0 {
                let before = to_diagram(hover);
                self.set_zoom(self.zoom * zoom_delta);
                self.pan = hover - response.rect.min - before.to_vec2() * self.zoom;
            } else if scroll != Vec2::ZERO {
                self.pan += scroll;
            }
        }

        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        let p = pointer.map(to_diagram);
        let dp = |p: Pos2| render::Point::new(f64::from(p.x), f64::from(p.y));
        let tolerance = f64::from(5.0 / self.zoom);

        // Hover cursor.
        if let (Some(p), Drag::None) = (p, &self.drag) {
            let icon = match tool {
                Tool::Node(_) => CursorIcon::Crosshair,
                Tool::Edge(_) => CursorIcon::Alias,
                Tool::Select if self.on_handle(scene, selection, p) => CursorIcon::ResizeNwSe,
                Tool::Select if scene.hit_test(dp(p), tolerance).is_some() => CursorIcon::Move,
                Tool::Select => CursorIcon::Default,
            };
            response.ctx.set_cursor_icon(icon);
        }

        // Middle or right drag pans in any tool.
        if response.dragged_by(PointerButton::Middle)
            || response.dragged_by(PointerButton::Secondary)
        {
            self.pan += response.drag_delta();
        }

        let pressed =
            response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed());
        if pressed && let Some(p) = p {
            let shift = ui.input(|i| i.modifiers.shift || i.modifiers.command);
            match tool {
                Tool::Select => {
                    if self.on_handle(scene, selection, p)
                        && let Some(view) = selection.first()
                        && let Some(rect) = scene.node_rect(view)
                    {
                        self.drag = Drag::Resize {
                            view: view.clone(),
                            rect,
                            to: p,
                        };
                    } else if let Some(hit) = scene.hit_test(dp(p), tolerance) {
                        let hit = hit.to_owned();
                        let mut sel: Vec<String> = selection.to_vec();
                        if shift {
                            if let Some(i) = sel.iter().position(|s| *s == hit) {
                                sel.remove(i);
                            } else {
                                sel.push(hit.clone());
                            }
                        } else if !sel.contains(&hit) {
                            sel = vec![hit.clone()];
                        }
                        if sel != selection {
                            out.commands.push(("edit.select", json!({ "ids": sel })));
                        }
                        let views: Vec<String> = sel
                            .iter()
                            .filter(|id| scene.node_rect(id).is_some())
                            .cloned()
                            .collect();
                        if scene.node_rect(&hit).is_some() && !views.is_empty() {
                            self.drag = Drag::Move {
                                views,
                                from: p,
                                to: p,
                            };
                        }
                    } else {
                        if !selection.is_empty() {
                            out.commands.push(("edit.select", json!({ "ids": [] })));
                        }
                        self.drag = Drag::Pan;
                    }
                }
                Tool::Node(spec) => {
                    let mut params = json!({ "kind": spec.id, "x": p.x.round(), "y": p.y.round() });
                    // Dropped on a box: create it inside (the engine decides whether it fits).
                    if let Some((container, _)) = scene.node_at(dp(p)) {
                        params["container"] = json!(container);
                    }
                    out.commands.push(("node.create", params));
                    out.tool_done = true;
                }
                Tool::Edge(_) => {
                    if let Some((tail, _)) = scene.node_at(dp(p)) {
                        self.drag = Drag::Edge {
                            tail: tail.to_owned(),
                            from: p,
                            to: p,
                        };
                    }
                }
            }
        }

        if response.dragged_by(PointerButton::Primary) {
            let delta = response.drag_delta() / self.zoom;
            match &mut self.drag {
                Drag::Pan => self.pan += response.drag_delta(),
                Drag::Move { to, .. } | Drag::Resize { to, .. } | Drag::Edge { to, .. } => {
                    *to += delta
                }
                Drag::None => {}
            }
        }

        if response.double_clicked()
            && let Some(p) = p
            && scene.node_at(dp(p)).is_some()
        {
            out.rename = true;
        }

        if ui.input(|i| i.pointer.primary_released()) {
            match std::mem::replace(&mut self.drag, Drag::None) {
                Drag::Move { views, from, to } => {
                    let d = to - from;
                    let (dx, dy) = (d.x.round(), d.y.round());
                    if dx != 0.0 || dy != 0.0 {
                        out.commands
                            .push(("view.move", json!({ "views": views, "dx": dx, "dy": dy })));
                    }
                }
                Drag::Resize { view, rect, to } => {
                    let w = (f64::from(to.x) - rect.x).max(20.0).round();
                    let h = (f64::from(to.y) - rect.y).max(20.0).round();
                    out.commands.push((
                        "view.resize",
                        json!({ "view": view, "width": w, "height": h }),
                    ));
                }
                Drag::Edge { tail, to, .. } => {
                    if let (Tool::Edge(spec), Some((head, _))) = (tool, scene.node_at(dp(to))) {
                        out.commands.push((
                            "edge.create",
                            json!({ "kind": spec.id, "tail": tail, "head": head }),
                        ));
                    }
                    out.tool_done = true;
                }
                Drag::Pan | Drag::None => {}
            }
        }

        self.paint_drag(&painter, scene, &to_screen);
        out
    }

    fn on_handle(&self, scene: &Scene, selection: &[String], p: Pos2) -> bool {
        let [only] = selection else { return false };
        let Some(r) = scene.node_rect(only) else {
            return false;
        };
        let reach = HANDLE / self.zoom;
        (p.x - r.right() as f32).abs() <= reach && (p.y - r.bottom() as f32).abs() <= reach
    }

    fn paint_scene(
        &self,
        painter: &egui::Painter,
        scene: &Scene,
        to_screen: &dyn Fn(f64, f64) -> Pos2,
    ) {
        let z = self.zoom;
        let col = |c: render::Color| Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3);
        let pts = |p: &[render::Point]| p.iter().map(|p| to_screen(p.x, p.y)).collect::<Vec<_>>();
        for prim in &scene.prims {
            match prim {
                Prim::Rect {
                    rect,
                    fill,
                    stroke,
                    dashed,
                } => {
                    let r = Rect::from_min_max(
                        to_screen(rect.x, rect.y),
                        to_screen(rect.right(), rect.bottom()),
                    );
                    if let Some(f) = fill {
                        painter.rect_filled(r, CornerRadius::ZERO, col(*f));
                    }
                    let stroke = Stroke::new(1.0, col(*stroke));
                    if *dashed {
                        let c = [
                            r.left_top(),
                            r.right_top(),
                            r.right_bottom(),
                            r.left_bottom(),
                            r.left_top(),
                        ];
                        painter.extend(Shape::dashed_line(&c, stroke, 6.0 * z, 4.0 * z));
                    } else {
                        painter.rect_stroke(r, CornerRadius::ZERO, stroke, StrokeKind::Middle);
                    }
                }
                Prim::Ellipse {
                    rect,
                    fill,
                    stroke,
                    dashed,
                } => {
                    let center = to_screen(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
                    let radius = Vec2::new(rect.w as f32 / 2.0, rect.h as f32 / 2.0) * z;
                    if let Some(f) = fill {
                        painter.add(Shape::ellipse_filled(center, radius, col(*f)));
                    }
                    let stroke = Stroke::new(1.0, col(*stroke));
                    if *dashed {
                        let outline: Vec<Pos2> = (0..=64)
                            .map(|i| {
                                let a = i as f32 / 64.0 * std::f32::consts::TAU;
                                center + Vec2::new(a.cos() * radius.x, a.sin() * radius.y)
                            })
                            .collect();
                        painter.extend(Shape::dashed_line(&outline, stroke, 6.0 * z, 4.0 * z));
                    } else {
                        painter.add(Shape::ellipse_stroke(center, radius, stroke));
                    }
                }
                Prim::Line {
                    points,
                    stroke,
                    dashed,
                } => {
                    let stroke = Stroke::new(1.0, col(*stroke));
                    if *dashed {
                        painter.extend(Shape::dashed_line(&pts(points), stroke, 6.0 * z, 4.0 * z));
                    } else {
                        painter.add(Shape::line(pts(points), stroke));
                    }
                }
                Prim::Polygon {
                    points,
                    fill,
                    stroke,
                } => {
                    painter.add(Shape::convex_polygon(
                        pts(points),
                        fill.map_or(Color32::TRANSPARENT, col),
                        Stroke::new(1.0, col(*stroke)),
                    ));
                }
                Prim::Text {
                    rect, text, style, ..
                } => {
                    let size = (style.size as f32 * z).max(1.0);
                    let mut format = TextFormat {
                        font_id: FontId::new(size, fonts::family(style.bold, style.italic)),
                        color: col(style.color),
                        ..Default::default()
                    };
                    if style.underline {
                        format.underline = Stroke::new(1.0, col(style.color));
                    }
                    let mut job = LayoutJob::default();
                    job.append(text, 0.0, format);
                    let galley = painter.layout_job(job);
                    painter.galley(to_screen(rect.x, rect.y), galley, Color32::BLACK);
                }
            }
        }
    }

    fn paint_selection(
        &self,
        painter: &egui::Painter,
        scene: &Scene,
        selection: &[String],
        to_screen: &dyn Fn(f64, f64) -> Pos2,
        tool: Tool,
    ) {
        let stroke = Stroke::new(2.0, SELECT);
        for hit in scene.hits.iter().filter(|h| selection.contains(&h.view)) {
            match &hit.shape {
                HitShape::Rect(r) => {
                    let rect =
                        Rect::from_min_max(to_screen(r.x, r.y), to_screen(r.right(), r.bottom()));
                    painter.rect_stroke(
                        rect.expand(1.0),
                        CornerRadius::ZERO,
                        stroke,
                        StrokeKind::Outside,
                    );
                    if selection.len() == 1 && tool == Tool::Select {
                        let h = Rect::from_center_size(rect.max, Vec2::splat(HANDLE));
                        painter.rect_filled(h, CornerRadius::ZERO, SELECT);
                    }
                }
                HitShape::Polyline(points) => {
                    let pts: Vec<Pos2> = points.iter().map(|p| to_screen(p.x, p.y)).collect();
                    painter.add(Shape::line(
                        pts.clone(),
                        Stroke::new(3.0, SELECT.gamma_multiply(0.5)),
                    ));
                    for p in pts {
                        painter.rect_filled(
                            Rect::from_center_size(p, Vec2::splat(5.0)),
                            CornerRadius::ZERO,
                            SELECT,
                        );
                    }
                }
            }
        }
    }

    fn paint_drag(
        &self,
        painter: &egui::Painter,
        scene: &Scene,
        to_screen: &dyn Fn(f64, f64) -> Pos2,
    ) {
        let ghost = Stroke::new(1.0, SELECT);
        let screen = |r: render::Rect| {
            Rect::from_min_max(to_screen(r.x, r.y), to_screen(r.right(), r.bottom()))
        };
        let dashed_rect = |r: Rect| {
            let c = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            painter.extend(Shape::dashed_line(&c, ghost, 4.0, 3.0));
        };
        match &self.drag {
            Drag::Move { views, from, to } => {
                let d = *to - *from;
                for v in views {
                    if let Some(r) = scene.node_rect(v) {
                        let moved =
                            render::Rect::new(r.x + f64::from(d.x), r.y + f64::from(d.y), r.w, r.h);
                        dashed_rect(screen(moved));
                    }
                }
            }
            Drag::Resize { rect, to, .. } => {
                let w = (f64::from(to.x) - rect.x).max(20.0);
                let h = (f64::from(to.y) - rect.y).max(20.0);
                dashed_rect(screen(render::Rect::new(rect.x, rect.y, w, h)));
            }
            Drag::Edge { tail, to, .. } => {
                if let Some(r) = scene.node_rect(tail) {
                    let c = r.center();
                    painter.extend(Shape::dashed_line(
                        &[
                            to_screen(c.x, c.y),
                            to_screen(f64::from(to.x), f64::from(to.y)),
                        ],
                        ghost,
                        4.0,
                        3.0,
                    ));
                }
            }
            Drag::Pan | Drag::None => {}
        }
    }
}
