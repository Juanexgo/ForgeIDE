use eframe::egui::{self, Key, Modifiers, ScrollArea, TextEdit};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Files,
    Commands,
    Symbols,
}

#[derive(Clone, Copy)]
pub enum Command {
    NewFile,
    OpenFile,
    OpenFolder,
    SaveFile,
    SaveAll,
    CloseTab,
    SearchWorkspace,
    ShowExplorer,
    GoToSymbol,
    ShowGit,
    ShowDebugger,
    ShowTests,
    ShowLanguageServer,
    ShowExtensions,
    ToggleTerminal,
    ToggleSidebar,
    ShowAccounts,
    ShowProblems,
    ToggleMinimap,
    ToggleSplit,
    Find,
    Replace,
    UndoBatch,
    RedoBatch,
    Restore,
}

pub struct Actions {
    pub file_open: bool,
    pub workspace_open: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone)]
pub enum Choice {
    File(PathBuf),
    Command(Command),
    ExtensionCommand(String),
    Line(usize),
}

const COMMANDS: &[(Command, &str, &str)] = &[
    (Command::NewFile, "New File", "File"),
    (Command::OpenFile, "Open File", "File"),
    (Command::OpenFolder, "Open Folder", "Workspace"),
    (Command::SaveFile, "Save File", "File"),
    (Command::SaveAll, "Save All", "File"),
    (Command::CloseTab, "Close Active Tab", "Editor"),
    (Command::Find, "Find in File", "Editor"),
    (Command::GoToSymbol, "Go to Symbol in File", "Editor"),
    (Command::Replace, "Replace in File", "Editor"),
    (Command::UndoBatch, "Undo", "Editor"),
    (Command::RedoBatch, "Redo", "Editor"),
    (Command::Restore, "Restore Unsaved Edits", "File"),
    (Command::SearchWorkspace, "Search Workspace", "Workspace"),
    (Command::ShowExplorer, "Show Explorer", "View"),
    (Command::ShowGit, "Show Source Control", "View"),
    (Command::ShowDebugger, "Show Debugger", "View"),
    (Command::ShowTests, "Show Testing", "View"),
    (Command::ShowLanguageServer, "Show Language Server", "View"),
    (Command::ShowExtensions, "Show Extensions", "View"),
    (Command::ToggleTerminal, "Toggle Terminal", "View"),
    (Command::ToggleSidebar, "Toggle Sidebar", "View"),
    (Command::ShowAccounts, "GitHub Accounts", "Account"),
    (Command::ShowProblems, "Show Problems", "View"),
    (Command::ToggleMinimap, "Toggle Minimap", "View"),
    (Command::ToggleSplit, "Toggle Split Editor", "View"),
];

fn available(command: Command, actions: &Actions) -> bool {
    match command {
        Command::SaveFile
        | Command::SaveAll
        | Command::CloseTab
        | Command::Find
        | Command::Replace
        | Command::GoToSymbol
        | Command::ToggleSplit => actions.file_open,
        Command::UndoBatch => actions.can_undo,
        Command::RedoBatch => actions.can_redo,
        Command::SearchWorkspace
        | Command::ShowDebugger
        | Command::ShowTests
        | Command::ShowLanguageServer => actions.workspace_open,
        _ => true,
    }
}

#[derive(Default)]
pub struct Palette {
    pub mode: Option<Mode>,
    query: String,
    selected: usize,
    focus: bool,
    symbols: Option<Vec<crate::syntax::Symbol>>,
    results: Option<Vec<Item>>,
}

impl Palette {
    pub fn open(&mut self, mode: Mode) {
        self.mode = Some(mode);
        self.query.clear();
        self.selected = 0;
        self.focus = true;
        self.symbols = None;
        self.refresh();
    }

    pub fn refresh(&mut self) {
        self.results = None;
    }

    pub fn show(
        &mut self,
        context: &egui::Context,
        root: Option<&Path>,
        files: impl Iterator<Item = PathBuf>,
        actions: Actions,
        active: Option<(&Path, &str)>,
        extension_commands: &[(String, String)],
    ) -> Option<Choice> {
        self.mode?;
        let mut choice = None;
        let mut files = Some(files);
        let modal = egui::Modal::new(egui::Id::new("workbench-palette")).show(context, |ui| {
            ui.set_width((context.screen_rect().width() - 90.0).clamp(280.0, 580.0));
            ui.horizontal(|ui| {
                for (mode, title) in [
                    (Mode::Files, "Files"),
                    (Mode::Commands, "Commands"),
                    (Mode::Symbols, "Symbols"),
                ] {
                    if ui
                        .selectable_label(self.mode == Some(mode), title)
                        .clicked()
                    {
                        self.open(mode);
                    }
                }
            });
            let mode = self.mode.expect("palette open");
            let field = ui.add(
                TextEdit::singleline(&mut self.query)
                    .hint_text(match mode {
                        Mode::Files => "Type a filename or path",
                        Mode::Commands => "Type a command",
                        Mode::Symbols => "Type a symbol name",
                    })
                    .desired_width(f32::INFINITY),
            );
            if std::mem::take(&mut self.focus) {
                field.request_focus();
            }
            if field.changed() {
                self.selected = 0;
                self.refresh();
            }

            let options = self.results.get_or_insert_with(|| match mode {
                Mode::Files => file_results(root, files.take().into_iter().flatten(), &self.query),
                Mode::Commands => {
                    let mut commands = command_results(&actions, &self.query);
                    if actions.file_open {
                        commands.extend(
                            extension_commands
                                .iter()
                                .filter(|(id, title)| {
                                    rank(
                                        &self.query.trim().to_lowercase(),
                                        &title.to_lowercase(),
                                        &id.to_lowercase(),
                                    )
                                    .is_some()
                                })
                                .map(|(id, title)| Item {
                                    title: title.clone(),
                                    detail: format!("Extension · {id}"),
                                    choice: Choice::ExtensionCommand(id.clone()),
                                }),
                        );
                    }
                    commands.truncate(100);
                    commands
                }
                Mode::Symbols => {
                    let symbols = self.symbols.get_or_insert_with(|| {
                        active
                            .map(|(path, source)| crate::syntax::symbols(path, source))
                            .unwrap_or_default()
                    });
                    let query = self.query.trim().to_lowercase();
                    symbols
                        .iter()
                        .filter(|symbol| {
                            rank(
                                &query,
                                &symbol.name.to_lowercase(),
                                &symbol.name.to_lowercase(),
                            )
                            .is_some()
                        })
                        .take(60)
                        .map(|symbol| Item {
                            title: symbol.name.clone(),
                            detail: format!("{} · line {}", symbol.kind, symbol.line),
                            choice: Choice::Line(symbol.line),
                        })
                        .collect()
                }
            });
            let navigate = field.has_focus() || field.lost_focus();
            let direction = if navigate {
                ui.input_mut(|input| {
                    if input.consume_key(Modifiers::NONE, Key::ArrowDown) {
                        1
                    } else if input.consume_key(Modifiers::NONE, Key::ArrowUp) {
                        -1
                    } else {
                        0
                    }
                })
            } else {
                0
            };
            self.selected = self
                .selected
                .saturating_add_signed(direction)
                .min(options.len().saturating_sub(1));
            let enter = navigate && ui.input(|input| input.key_pressed(Key::Enter));
            let mut picked = enter.then_some(self.selected);
            if options.is_empty() {
                ui.label(crate::chrome::small("No matches"));
            }
            ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                for (index, item) in options.iter().enumerate() {
                    let is_selected = index == self.selected;
                    let row =
                        crate::chrome::palette_row(ui, &item.title, &item.detail, is_selected);
                    if row.clicked() {
                        picked = Some(index);
                    }
                    if direction != 0 && is_selected {
                        row.scroll_to_me(None);
                    }
                }
            });
            ui.label(crate::chrome::small(
                "↑ ↓ Navigate     Enter Open     Esc Close",
            ));
            choice = picked.and_then(|index| options.get(index).map(|item| item.choice.clone()));
        });
        if choice.is_some() || modal.should_close() {
            self.mode = None;
        }
        choice
    }
}

struct Item {
    title: String,
    detail: String,
    choice: Choice,
}

fn file_results(
    root: Option<&Path>,
    files: impl Iterator<Item = PathBuf>,
    query: &str,
) -> Vec<Item> {
    let query = query.trim().to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut ranked = files
        .filter(|path| seen.insert(path.clone()))
        .filter_map(|path| {
            let title = path.file_name()?.to_string_lossy().into_owned();
            let detail = root
                .and_then(|root| path.strip_prefix(root).ok())
                .unwrap_or(&path)
                .parent()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            let full = if detail.is_empty() {
                title.clone()
            } else {
                format!("{detail}/{title}")
            };
            let score = rank(&query, &title.to_lowercase(), &full.to_lowercase())?;
            Some((
                score,
                Item {
                    title,
                    detail,
                    choice: Choice::File(path),
                },
            ))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.title.cmp(&right.1.title))
            .then(left.1.detail.cmp(&right.1.detail))
    });
    ranked.into_iter().take(50).map(|(_, item)| item).collect()
}

fn command_results(actions: &Actions, query: &str) -> Vec<Item> {
    let query = query.trim().to_lowercase();
    let mut ranked = COMMANDS
        .iter()
        .filter_map(|(command, title, detail)| {
            if !available(*command, actions) {
                return None;
            }
            let score = rank(
                &query,
                &title.to_lowercase(),
                &format!("{detail} {title}").to_lowercase(),
            )?;
            Some((
                score,
                Item {
                    title: (*title).to_owned(),
                    detail: (*detail).to_owned(),
                    choice: Choice::Command(*command),
                },
            ))
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(score, _)| *score);
    ranked.into_iter().map(|(_, item)| item).collect()
}

fn rank(query: &str, title: &str, full: &str) -> Option<(u8, usize)> {
    if title.starts_with(query) {
        return Some((0, title.len()));
    }
    if let Some(offset) = title.find(query) {
        return Some((1, offset));
    }
    if let Some(offset) = full.find(query) {
        return Some((2, offset));
    }
    let mut remaining = full;
    let mut span = 0;
    for character in query.chars() {
        let end = remaining.find(character)? + character.len_utf8();
        span += end;
        remaining = &remaining[end..];
    }
    Some((3, span))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_filename_prefix_first_and_deduplicates_open_files() {
        let root = Path::new("workspace");
        let exact = root.join("src/config.rs");
        let root_match = root.join("config/default.rs");
        let options = file_results(
            Some(root),
            [root_match, exact.clone(), exact.clone()].into_iter(),
            "conf",
        );
        assert_eq!(options.len(), 2);
        assert!(matches!(&options[0].choice, Choice::File(path) if *path == exact));
        assert!(rank("gldb", "show", "show debugger").is_none());
        assert!(rank("tgtrm", "toggle terminal", "toggle terminal").is_some());
    }
}
