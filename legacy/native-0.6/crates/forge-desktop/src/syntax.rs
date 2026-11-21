use eframe::egui::{Color32, FontId, TextFormat, text::LayoutJob};

use std::{path::Path, sync::LazyLock};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

const MAX_HIGHLIGHT_BYTES: usize = 128 * 1024;
const CATEGORIES: &[&str] = &[
    "comment", "string", "constant", "keyword", "function", "type", "property", "operator",
];
const COLORS: &[Color32] = &[
    Color32::from_rgb(145, 140, 169),
    Color32::from_rgb(158, 220, 179),
    Color32::from_rgb(247, 193, 149),
    Color32::from_rgb(202, 169, 255),
    Color32::from_rgb(145, 183, 255),
    Color32::from_rgb(132, 217, 220),
    Color32::from_rgb(244, 182, 211),
    Color32::from_rgb(169, 163, 201),
];

const TYPESCRIPT_COMMON: &str = r#"
  (comment) @comment
  (string) @string
  (template_string) @string
  [(number) (true) (false) (null) (undefined)] @constant
  ["function" "return" "const" "let" "var" "import" "export" "from"
   "if" "else" "for" "while" "new" "class" "extends" "async" "await"
   "throw" "try" "catch" "switch" "case" "break" "continue"] @keyword
  (function_declaration name: (identifier) @function)
  (call_expression function: (identifier) @function)
"#;

static RUST: LazyLock<HighlightConfiguration> = LazyLock::new(|| {
    let mut config = HighlightConfiguration::new(
        tree_sitter_rust::LANGUAGE.into(),
        "rust",
        tree_sitter_rust::HIGHLIGHTS_QUERY,
        "",
        "",
    )
    .expect("the bundled Rust queries must match the pinned grammar");
    config.configure(CATEGORIES);
    config
});

static TYPESCRIPT: LazyLock<HighlightConfiguration> = LazyLock::new(|| {
    typescript(
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "typescript",
    )
});
static TSX: LazyLock<HighlightConfiguration> =
    LazyLock::new(|| typescript(tree_sitter_typescript::LANGUAGE_TSX.into(), "tsx"));

fn typescript(language: tree_sitter::Language, name: &str) -> HighlightConfiguration {
    let mut config = HighlightConfiguration::new(
        language,
        name,
        &format!(
            "{TYPESCRIPT_COMMON}\n{}",
            tree_sitter_typescript::HIGHLIGHTS_QUERY
        ),
        "",
        tree_sitter_typescript::LOCALS_QUERY,
    )
    .expect("the bundled TypeScript queries must match the pinned grammar");
    config.configure(CATEGORIES);
    config
}

pub fn layout(path: &Path, source: &str, foreground: Color32) -> LayoutJob {
    let config = if source.len() > MAX_HIGHLIGHT_BYTES {
        None
    } else {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("rs") => Some(&*RUST),
            Some("ts" | "mts" | "cts") => Some(&*TYPESCRIPT),
            Some("tsx") => Some(&*TSX),
            _ => None,
        }
    };

    config
        .and_then(|config| highlighted(config, source, foreground))
        .unwrap_or_else(|| {
            let mut job = LayoutJob::default();
            job.append(source, 0.0, code_format(foreground));
            job.wrap.max_width = f32::INFINITY;
            job
        })
}

fn code_format(color: Color32) -> TextFormat {
    TextFormat {
        line_height: Some(crate::chrome::font_size() * 1.53),
        ..TextFormat::simple(FontId::monospace(crate::chrome::font_size()), color)
    }
}

fn highlighted(
    config: &HighlightConfiguration,
    source: &str,
    foreground: Color32,
) -> Option<LayoutJob> {
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(config, source.as_bytes(), None, |_| None)
        .ok()?;
    let mut stack = Vec::new();
    let mut job = LayoutJob::default();
    for event in events {
        match event.ok()? {
            HighlightEvent::HighlightStart(highlight) => {
                let color = COLORS[highlight.0];
                let color = if foreground.r() < 100 {
                    Color32::from_rgb(
                        color.r().saturating_sub(85),
                        color.g().saturating_sub(85),
                        color.b().saturating_sub(65),
                    )
                } else {
                    color
                };
                stack.push(crate::chrome::syntax_color(CATEGORIES[highlight.0]).unwrap_or(color));
            }
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let color = stack.last().copied().unwrap_or(foreground);
                job.append(source.get(start..end)?, 0.0, code_format(color));
            }
        }
    }
    job.wrap.max_width = f32::INFINITY;
    Some(job)
}

pub struct Symbol {
    pub name: String,
    pub kind: &'static str,
    pub line: usize,
}

pub fn symbols(path: &Path, source: &str) -> Vec<Symbol> {
    const SYMBOL_SOURCE_LIMIT: usize = 512 * 1024;
    if source.len() > SYMBOL_SOURCE_LIMIT {
        return Vec::new();
    }
    let language = match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => tree_sitter_rust::LANGUAGE.into(),
        Some("ts" | "mts" | "cts") => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Some("tsx") => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => return Vec::new(),
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let mut stack = vec![tree.root_node()];
    let mut symbols = Vec::new();
    while let Some(node) = stack.pop() {
        let kind = match node.kind() {
            "function_item" | "function_declaration" | "function_signature" => Some("Function"),
            "method_definition" | "method_signature" => Some("Method"),
            "struct_item" => Some("Struct"),
            "enum_item" | "enum_declaration" => Some("Enum"),
            "trait_item" => Some("Trait"),
            "interface_declaration" => Some("Interface"),
            "class_declaration" => Some("Class"),
            "type_item" | "type_alias_declaration" => Some("Type"),
            "const_item" => Some("Constant"),
            "mod_item" => Some("Module"),
            _ => None,
        };
        if let Some(kind) = kind {
            if let Some(name) = node
                .child_by_field_name("name")
                .and_then(|name| source.get(name.byte_range()))
            {
                symbols.push(Symbol {
                    name: name.to_owned(),
                    kind,
                    line: node.start_position().row + 1,
                });
                if symbols.len() >= 1_000 {
                    break;
                }
            }
        }
        for index in (0..node.named_child_count()).rev() {
            if let Some(child) = node.named_child(index) {
                stack.push(child);
            }
        }
    }
    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_declarations_without_requesting_a_language_server() {
        let source = "interface Account { id: number }\nfunction make(): number { return 7; }";
        let result = symbols(Path::new("sample.ts"), source);
        assert_eq!(result.len(), 2);
        assert_eq!(
            (&*result[0].name, result[0].kind, result[0].line),
            ("Account", "Interface", 1)
        );
        assert_eq!(
            (&*result[1].name, result[1].kind, result[1].line),
            ("make", "Function", 2)
        );
    }

    #[test]
    fn bundled_grammars_highlight_and_preserve_source() {
        for (path, code) in [
            ("main.rs", "fn main() { let message = \"🦀\"; }"),
            ("main.ts", "function number(): number { return 3; }"),
            ("main.tsx", "const example: number = 3;"),
        ] {
            let job = layout(Path::new(path), code, Color32::WHITE);
            assert_eq!(job.text, code);
            assert!(
                job.sections
                    .iter()
                    .any(|section| section.format.color != Color32::WHITE)
            );
        }
    }
}
