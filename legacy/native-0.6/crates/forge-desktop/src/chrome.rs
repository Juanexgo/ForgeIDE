use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    TextStyle, Ui, WidgetText, pos2, vec2,
};
use std::path::Path;

pub fn theme_rail() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("rail").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(232, 234, 241)
    } else {
        Color32::from_rgb(19, 18, 24)
    }
}
pub fn theme_panel() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("panel").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(244, 245, 249)
    } else {
        Color32::from_rgb(25, 24, 32)
    }
}
pub fn theme_editor() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("editor").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(252, 252, 254)
    } else {
        Color32::from_rgb(18, 17, 23)
    }
}
pub fn theme_elevated() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("elevated").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(232, 234, 242)
    } else {
        Color32::from_rgb(36, 34, 46)
    }
}
pub fn theme_border() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("border").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(207, 211, 224)
    } else {
        Color32::from_rgb(46, 43, 56)
    }
}
pub fn theme_text() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("text").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(35, 39, 55)
    } else {
        Color32::from_rgb(227, 224, 239)
    }
}
pub fn theme_muted() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("muted").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(82, 89, 110)
    } else {
        Color32::from_rgb(161, 156, 180)
    }
}
pub fn theme_dim() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("dim").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(109, 116, 138)
    } else {
        Color32::from_rgb(125, 119, 149)
    }
}
pub fn theme_accent() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("accent").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(96, 65, 180)
    } else {
        Color32::from_rgb(186, 164, 255)
    }
}
pub fn theme_active() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("active").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(227, 218, 248)
    } else {
        Color32::from_rgb(49, 40, 71)
    }
}
pub fn theme_green() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("green").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(30, 122, 74)
    } else {
        Color32::from_rgb(151, 224, 187)
    }
}
pub fn theme_red() -> Color32 {
    if let Some(color) = CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.colors.get("red").copied())
    }) {
        return Color32::from_rgb(color[0], color[1], color[2]);
    }
    if LIGHT.with(std::cell::Cell::get) {
        Color32::from_rgb(182, 48, 71)
    } else {
        Color32::from_rgb(255, 159, 171)
    }
}

thread_local! {
    static CUSTOM: std::cell::RefCell<Option<crate::packages::Theme>> = const { std::cell::RefCell::new(None) };
    static LIGHT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FONT_SIZE: std::cell::Cell<f32> = const { std::cell::Cell::new(15.0) };
}

pub fn set_theme(theme: Option<crate::packages::Theme>) {
    CUSTOM.with(|custom| *custom.borrow_mut() = theme);
}

pub fn syntax_color(category: &str) -> Option<Color32> {
    CUSTOM.with(|theme| {
        theme
            .borrow()
            .as_ref()
            .and_then(|theme| theme.syntax.get(category))
            .map(|color| Color32::from_rgb(color[0], color[1], color[2]))
    })
}

pub fn font_size() -> f32 {
    FONT_SIZE.with(std::cell::Cell::get)
}

pub fn configure(context: &egui::Context, light: bool, font_size: f32, compact: bool, zoom: f32) {
    let light = CUSTOM.with(|theme| theme.borrow().as_ref().map_or(light, |theme| !theme.dark));
    LIGHT.with(|value| value.set(light));
    FONT_SIZE.with(|value| value.set(font_size));
    context.set_zoom_factor(zoom);
    let mut style = (*context.style()).clone();
    style.spacing.item_spacing.y = if compact { 4.0 } else { 7.0 };
    style.spacing.button_padding.y = if compact { 3.0 } else { 5.0 };
    style.visuals = visuals(light);
    context.set_style(style);
}

fn visuals(light: bool) -> egui::Visuals {
    let mut visuals = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    visuals.override_text_color = Some(theme_text());
    visuals.panel_fill = theme_panel();
    visuals.window_fill = theme_panel();
    visuals.window_stroke = Stroke::new(1.0, theme_border());
    visuals.extreme_bg_color = theme_editor();
    visuals.faint_bg_color = theme_elevated();
    visuals.selection.bg_fill = theme_active();
    visuals.selection.stroke = Stroke::new(1.0, theme_accent());
    visuals.hyperlink_color = theme_accent();
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = 6.into();
        widget.bg_stroke = Stroke::new(1.0, theme_border());
        widget.fg_stroke.color = theme_text();
    }
    visuals.widgets.noninteractive.fg_stroke.color = theme_muted();
    visuals.widgets.inactive.weak_bg_fill = theme_elevated();
    visuals.widgets.inactive.bg_fill = theme_elevated();
    visuals.widgets.hovered.weak_bg_fill = theme_active();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme_dim());
    visuals.widgets.active.weak_bg_fill = theme_active();
    visuals
}

pub fn apply(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes, family) in [
        (
            "Inter",
            &include_bytes!("../assets/Inter.ttf")[..],
            egui::FontFamily::Proportional,
        ),
        (
            "IBM Plex Mono",
            &include_bytes!("../assets/IBMPlexMono-Regular.ttf")[..],
            egui::FontFamily::Monospace,
        ),
    ] {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.into());
    }
    context.set_fonts(fonts);
    let mut style = (*context.style()).clone();
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(11.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(19.0));
    style.spacing.item_spacing = vec2(7.0, 7.0);
    style.spacing.button_padding = vec2(9.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.indent = 15.0;

    style.visuals = visuals(false);
    context.set_style(style);
}

pub fn panel(fill: Color32, horizontal: i8, vertical: i8) -> Frame {
    Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(horizontal, vertical))
}

pub fn label(text: impl Into<String>) -> RichText {
    RichText::new(text).size(11.0).strong().color(theme_muted())
}

pub fn small(text: impl Into<String>) -> RichText {
    RichText::new(text).size(11.0).color(theme_muted())
}

#[derive(Clone, Copy)]
pub enum Icon {
    Forge,
    Files,
    Search,
    Git,
    Code,
    Debug,
    Extensions,
    Terminal,
    Split,
    Close,
    Tests,
    Account,
}

pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    active: bool,
    size: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if active || response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect,
            7.0,
            if active {
                theme_active()
            } else {
                theme_elevated()
            },
        );
    }
    if active {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.left_center() - vec2(0.0, 9.0), vec2(2.0, 18.0)),
            1.0,
            theme_accent(),
        );
    }
    symbol(
        ui,
        rect.center(),
        icon,
        if active {
            theme_accent()
        } else {
            theme_muted()
        },
    );
    response.on_hover_text(label)
}

pub fn symbol(ui: &Ui, center: Pos2, icon: Icon, color: Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.45, color);
    let p = |x, y| center + vec2(x, y);
    let segment = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    let rectangle = |a: (f32, f32), b: (f32, f32)| {
        painter.rect_stroke(
            Rect::from_min_max(p(a.0, a.1), p(b.0, b.1)),
            2.0,
            stroke,
            StrokeKind::Middle,
        );
    };
    match icon {
        Icon::Forge => {
            painter.add(egui::Shape::closed_line(
                vec![
                    p(0.0, -8.0),
                    p(8.0, -3.5),
                    p(8.0, 4.5),
                    p(0.0, 9.0),
                    p(-8.0, 4.5),
                    p(-8.0, -3.5),
                ],
                stroke,
            ));
            segment((-8.0, -3.5), (0.0, 1.0));
            segment((0.0, 1.0), (8.0, -3.5));
            segment((0.0, 1.0), (0.0, 9.0));
        }
        Icon::Files => {
            rectangle((-5.0, -4.0), (7.0, 8.0));
            segment((-8.0, 4.0), (-8.0, -8.0));
            segment((-8.0, -8.0), (3.0, -8.0));
        }
        Icon::Search => {
            painter.circle_stroke(p(-1.5, -1.5), 5.6, stroke);
            segment((2.8, 2.8), (8.0, 8.0));
        }
        Icon::Git => {
            segment((-5.0, -4.0), (-5.0, 4.0));
            segment((-4.0, 2.0), (4.0, -2.0));
            for (x, y) in [(-5.0, -6.0), (-5.0, 6.0), (5.0, -4.0)] {
                painter.circle_stroke(p(x, y), 2.15, stroke);
            }
        }
        Icon::Code => {
            segment((-3.0, -5.0), (-8.0, 0.0));
            segment((-8.0, 0.0), (-3.0, 5.0));
            segment((3.0, -5.0), (8.0, 0.0));
            segment((8.0, 0.0), (3.0, 5.0));
            segment((1.5, -7.0), (-1.5, 7.0));
        }
        Icon::Debug => {
            painter.add(egui::Shape::closed_line(
                vec![p(-4.0, -7.0), p(8.0, 0.0), p(-4.0, 7.0)],
                stroke,
            ));
            segment((-8.0, -5.0), (-8.0, 5.0));
        }
        Icon::Extensions => {
            for (x, y) in [(-7.5, -7.5), (1.0, -7.5), (-7.5, 1.0), (1.0, 1.0)] {
                rectangle((x, y), (x + 6.5, y + 6.5));
            }
        }
        Icon::Terminal => {
            rectangle((-8.5, -6.5), (8.5, 6.5));
            segment((-5.5, -3.0), (-2.0, 0.0));
            segment((-2.0, 0.0), (-5.5, 3.0));
            segment((0.0, 3.0), (5.5, 3.0));
        }
        Icon::Split => {
            rectangle((-8.0, -6.5), (8.0, 6.5));
            segment((0.0, -6.5), (0.0, 6.5));
        }
        Icon::Close => {
            segment((-3.0, -3.0), (3.0, 3.0));
            segment((3.0, -3.0), (-3.0, 3.0));
        }
        Icon::Account => {
            painter.circle_stroke(p(0.0, -4.0), 3.6, stroke);
            painter.add(egui::Shape::line(
                vec![
                    p(-7.0, 8.0),
                    p(-6.0, 4.0),
                    p(-3.0, 1.0),
                    p(3.0, 1.0),
                    p(6.0, 4.0),
                    p(7.0, 8.0),
                ],
                stroke,
            ));
        }
        Icon::Tests => {
            painter.circle_stroke(center, 8.0, stroke);
            segment((-4.0, 0.0), (-1.0, 3.0));
            segment((-1.0, 3.0), (5.0, -3.0));
        }
    }
}

pub fn file_kind(path: &Path) -> (&'static str, &'static str, Color32) {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => ("RS", "Rust", Color32::from_rgb(245, 170, 133)),
        Some("ts" | "tsx" | "mts" | "cts") => {
            ("TS", "TypeScript", Color32::from_rgb(141, 184, 255))
        }
        Some("js" | "jsx" | "mjs" | "cjs") => {
            ("JS", "JavaScript", Color32::from_rgb(247, 218, 140))
        }
        Some("json") => ("{}", "JSON", Color32::from_rgb(240, 208, 138)),
        Some("toml" | "yml" | "yaml") => ("··", "Configuration", Color32::from_rgb(237, 181, 146)),
        Some("md") => ("M", "Markdown", theme_accent()),
        _ => ("·", "Plain text", theme_muted()),
    }
}

pub fn file_mark(ui: &mut Ui, path: &Path) {
    let (kind, _, color) = file_kind(path);
    let (rect, _) = ui.allocate_exact_size(vec2(22.0, 19.0), Sense::hover());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        kind,
        FontId::monospace(10.0),
        color,
    );
}

pub const EXPLORER_ROW_HEIGHT: f32 = 24.0;

pub fn folder_row(ui: &mut Ui, name: &str, expanded: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), EXPLORER_ROW_HEIGHT),
        Sense::click(),
    );
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, expanded, name));
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if response.hovered() || response.has_focus() {
        painter.rect_filled(rect, 5.0, theme_elevated());
    }
    let pivot = rect.left_center() + vec2(8.0, 0.0);
    let points = if expanded {
        [
            pivot + vec2(-3.0, -1.5),
            pivot + vec2(0.0, 1.5),
            pivot + vec2(3.0, -1.5),
        ]
    } else {
        [
            pivot + vec2(-1.0, -3.0),
            pivot + vec2(2.0, 0.0),
            pivot + vec2(-1.0, 3.0),
        ]
    };
    painter.add(egui::Shape::line(
        points.to_vec(),
        Stroke::new(1.2, theme_muted()),
    ));
    let icon = rect.left_center() + vec2(23.0, 0.0);
    painter.rect_stroke(
        Rect::from_center_size(icon + vec2(0.0, 1.0), vec2(11.0, 7.0)),
        1.5,
        Stroke::new(1.0, theme_accent()),
        StrokeKind::Middle,
    );
    painter.line_segment(
        [icon + vec2(-5.5, -3.0), icon + vec2(-1.0, -3.0)],
        Stroke::new(1.0, theme_accent()),
    );
    paint_label(
        ui,
        &painter,
        rect,
        name,
        36.0,
        if expanded {
            theme_text()
        } else {
            theme_muted()
        },
    );
    response
}

pub fn file_row(ui: &mut Ui, path: &Path, name: &str, active: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), EXPLORER_ROW_HEIGHT),
        Sense::click(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if active || response.hovered() || response.has_focus() {
        painter.rect_filled(
            rect,
            5.0,
            if active {
                theme_active()
            } else {
                theme_elevated()
            },
        );
    }
    let (kind, _, color) = file_kind(path);
    painter.text(
        rect.left_center() + vec2(17.0, 0.0),
        Align2::CENTER_CENTER,
        kind,
        FontId::monospace(10.0),
        color,
    );
    paint_label(
        ui,
        &painter,
        rect,
        name,
        34.0,
        if active { theme_text() } else { theme_muted() },
    );
    response.on_hover_text(path.to_string_lossy())
}

fn paint_label(
    ui: &Ui,
    painter: &egui::Painter,
    rect: Rect,
    text: &str,
    inset: f32,
    color: Color32,
) {
    let galley = WidgetText::from(RichText::new(text).color(color)).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (rect.width() - inset - 4.0).max(0.0),
        TextStyle::Body,
    );
    painter.galley(
        pos2(rect.min.x + inset, rect.center().y - galley.size().y / 2.0),
        galley,
        theme_text(),
    );
}

pub fn palette_row(ui: &mut Ui, title: &str, detail: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 43.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, title)
    });
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if selected || response.hovered() || response.has_focus() {
        painter.rect_filled(
            rect,
            7.0,
            if selected {
                theme_active()
            } else {
                theme_elevated()
            },
        );
    }
    let title_box = rect.translate(vec2(0.0, if detail.is_empty() { 0.0 } else { -7.0 }));
    paint_label(ui, &painter, title_box, title, 11.0, theme_text());
    if !detail.is_empty() {
        let layout = WidgetText::from(small(detail)).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (rect.width() - 22.0).max(0.0),
            TextStyle::Small,
        );
        painter.galley(
            pos2(rect.min.x + 11.0, rect.center().y + 3.0),
            layout,
            theme_muted(),
        );
    }
    response
}

pub fn right(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    ui.with_layout(egui::Layout::right_to_left(Align::Center), contents);
}
