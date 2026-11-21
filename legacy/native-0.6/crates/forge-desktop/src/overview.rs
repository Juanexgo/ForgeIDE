use crate::chrome::{
    theme_accent, theme_active, theme_border, theme_dim, theme_editor, theme_panel,
};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};

pub fn show(
    ui: &mut Ui,
    galley: &egui::Galley,
    visible: std::ops::Range<usize>,
    active: Option<usize>,
) -> Option<usize> {
    let (rect, response) = ui.allocate_exact_size(
        vec2(89.0, ui.available_height().max(30.0)),
        Sense::click_and_drag(),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Jump in file overview"));
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0.0, theme_panel());
    painter.vline(rect.min.x, rect.y_range(), Stroke::new(1.0, theme_border()));

    let rows = &galley.rows;
    if rows.is_empty() {
        return None;
    }
    let canvas = rect.shrink2(vec2(9.0, 9.0));
    let bin_count = rows.len().min((canvas.height() / 3.0) as usize).max(1);
    let step = (canvas.height() / bin_count as f32).clamp(1.0, 4.0);
    let row_y =
        |row: usize| canvas.min.y + (row as f32 / rows.len() as f32) * bin_count as f32 * step;
    let viewport_top = row_y(visible.start.min(rows.len() - 1));
    let viewport_bottom = row_y(visible.end.min(rows.len())).max(viewport_top + 5.0);
    painter.rect_filled(
        Rect::from_min_max(
            pos2(rect.min.x + 3.0, viewport_top),
            pos2(rect.max.x - 4.0, viewport_bottom),
        ),
        3.0,
        if response.hovered() {
            theme_active()
        } else {
            theme_editor()
        },
    );
    for bin in 0..bin_count {
        let start = bin * rows.len() / bin_count;
        let end = (bin + 1) * rows.len() / bin_count;
        let representative = rows[start..end]
            .iter()
            .step_by((end - start).div_ceil(3).max(1))
            .max_by_key(|row| row.glyphs.len())
            .expect("nonempty bucket");
        let length = (representative.rect.width() * 0.18).min(canvas.width());
        if length < 1.0 {
            continue;
        }
        let depth = representative
            .glyphs
            .iter()
            .take_while(|glyph| glyph.chr == ' ' || glyph.chr == '\t')
            .count();
        let indent = (depth as f32 * 1.4).min(length * 0.75);
        let y = canvas.min.y + bin as f32 * step + step / 2.0;
        let is_current = active.is_some_and(|row| (start..end).contains(&row));
        painter.hline(
            (canvas.min.x + indent)..=(canvas.min.x + length),
            y,
            Stroke::new(
                if is_current { 1.7 } else { 1.1 },
                if is_current {
                    theme_accent()
                } else {
                    Color32::from_rgba_unmultiplied(
                        theme_dim().r(),
                        theme_dim().g(),
                        theme_dim().b(),
                        168,
                    )
                },
            ),
        );
    }
    let pointer = response
        .interact_pointer_pos()
        .filter(|_| response.clicked() || response.dragged())?;
    let fraction = ((pointer.y - canvas.min.y) / (bin_count as f32 * step)).clamp(0.0, 1.0);
    Some(((fraction * rows.len() as f32) as usize).min(rows.len() - 1) + 1)
}

pub fn visible_rows(
    output: &egui::text_edit::TextEditOutput,
    clip: Rect,
) -> std::ops::Range<usize> {
    let origin = output.galley_pos.y;
    let rows = &output.galley.rows;
    let first = rows.partition_point(|row| origin + row.rect.max.y < clip.min.y);
    let end = rows.partition_point(|row| origin + row.rect.min.y <= clip.max.y);
    first..end
}
