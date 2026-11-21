use eframe::egui::{self, text::LayoutJob};
use std::{collections::BTreeSet, ops::Range, path::Path};

#[derive(Default)]
pub struct Folds {
    source: String,
    pub regions: Vec<Range<usize>>,
    collapsed: BTreeSet<usize>,
}

impl Folds {
    pub fn sync(&mut self, path: &Path, source: &str) {
        if source.len() > 512 * 1024 {
            self.source.clear();
            self.collapsed.clear();
            self.regions.clear();
            return;
        }
        if self.source == source {
            return;
        }
        self.source = source.to_owned();
        self.collapsed.clear();
        self.regions.clear();
        if source.len() > 512 * 1024 {
            return;
        }
        let language = match path.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => tree_sitter_rust::LANGUAGE.into(),
            Some("ts" | "js" | "mts" | "cts") => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Some("tsx" | "jsx") => tree_sitter_typescript::LANGUAGE_TSX.into(),
            _ => return,
        };
        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&language).is_err() {
            return;
        }
        let Some(tree) = parser.parse(source, None) else {
            return;
        };
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            let start = node.start_position().row;
            let end = node.end_position().row;
            if node.is_named() && end > start + 1 && node.parent().is_some() {
                self.regions.push(start..end);
            }
            let mut cursor = node.walk();
            stack.extend(node.named_children(&mut cursor));
        }
        // Prefer the widest syntax node beginning on each line.
        self.regions
            .sort_by_key(|range| (range.start, std::cmp::Reverse(range.end)));
        self.regions.dedup_by_key(|range| range.start);
    }

    pub fn toggle(&mut self, row: usize) {
        if !self.collapsed.remove(&row) {
            self.collapsed.insert(row);
        }
    }

    pub fn unfold_all(&mut self) {
        self.collapsed.clear();
    }

    pub fn collapsed(&self, row: usize) -> bool {
        self.collapsed.contains(&row)
    }

    pub fn reveal(&mut self, row: usize) {
        for region in &self.regions {
            if region.start < row && row < region.end {
                self.collapsed.remove(&region.start);
            }
        }
    }

    pub fn hidden(&self) -> Vec<Range<usize>> {
        self.regions
            .iter()
            .filter(|region| self.collapsed.contains(&region.start))
            .map(|region| region.start + 1..region.end)
            .collect()
    }
}

/// Preserve source offsets and cursor positions by hiding layout sections, never source text.
pub fn apply(job: &mut LayoutJob, hidden: &[Range<usize>]) {
    if hidden.is_empty() {
        return;
    }
    let mut ranges = Vec::new();
    for (line, (start, contents)) in job
        .text
        .split_inclusive('\n')
        .scan(0usize, |offset, contents| {
            let start = *offset;
            *offset += contents.len();
            Some((start, contents))
        })
        .enumerate()
    {
        if hidden.iter().any(|range| range.contains(&line)) {
            ranges.push(start..start + contents.len());
        }
    }
    let sections = std::mem::take(&mut job.sections);
    for section in sections {
        let mut boundaries = vec![section.byte_range.start, section.byte_range.end];
        for range in &ranges {
            boundaries.extend(
                [range.start, range.end]
                    .into_iter()
                    .filter(|position| section.byte_range.contains(position)),
            );
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        for pair in boundaries.windows(2) {
            let mut part = section.clone();
            part.byte_range = pair[0]..pair[1];
            if ranges.iter().any(|range| range.contains(&pair[0])) {
                part.format.font_id.size = 0.001;
                part.format.line_height = Some(0.0);
                part.format.color = egui::Color32::TRANSPARENT;
            }
            job.sections.push(part);
        }
    }
}

pub fn gutter(
    ui: &egui::Ui,
    rect: egui::Rect,
    output: &egui::text_edit::TextEditOutput,
    folds: &mut Folds,
) -> bool {
    let mut changed = false;
    for region in &folds.regions {
        let Some(row) = output.galley.rows.get(region.start) else {
            continue;
        };
        if row.rect.height() < 1.0 {
            continue;
        }
        let y = output.galley_pos.y + row.rect.center().y;
        let hit = egui::Rect::from_center_size(
            egui::pos2(rect.min.x + 22.0, y),
            egui::vec2(13.0, row.rect.height()),
        );
        if !ui.clip_rect().intersects(hit) {
            continue;
        }
        let response = ui.interact(
            hit,
            output.response.id.with(("fold", region.start)),
            egui::Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                true,
                folds.collapsed(region.start),
                format!("Fold block at line {}", region.start + 1),
            )
        });
        ui.painter().text(
            hit.center(),
            egui::Align2::CENTER_CENTER,
            if folds.collapsed(region.start) {
                "›"
            } else {
                "⌄"
            },
            egui::FontId::proportional(13.0),
            crate::chrome::theme_muted(),
        );
        if response.clicked() {
            let start = region.start;
            folds.toggle(start);
            changed = true;
            break;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folded_layout_keeps_source_and_reveals_cursor() {
        let text = "fn main() {\n    let answer = 42;\n    println!(\"{answer}\");\n}\n";
        let mut folds = Folds::default();
        folds.sync(Path::new("main.rs"), text);
        assert!(folds.regions.iter().any(|range| range.start == 0));
        folds.toggle(0);
        let mut job = crate::syntax::layout(Path::new("main.rs"), text, egui::Color32::WHITE);
        apply(&mut job, &folds.hidden());
        assert_eq!(job.text, text);
        assert!(
            job.sections
                .iter()
                .any(|part| part.format.line_height == Some(0.0))
        );
        let context = egui::Context::default();
        let _ = context.run(egui::RawInput::default(), |ctx| {
            let plain = crate::syntax::layout(Path::new("main.rs"), text, egui::Color32::WHITE);
            let plain = ctx.fonts(|fonts| fonts.layout_job(plain));
            let folded = ctx.fonts(|fonts| fonts.layout_job(job.clone()));
            assert!(folded.size().y < plain.size().y - 20.0);
            assert_eq!(folded.rows.len(), plain.rows.len());
        });
        folds.reveal(1);
        assert!(folds.hidden().is_empty());
    }
}
