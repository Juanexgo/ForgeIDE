use eframe::egui::{self, Event, FontId, Key, Modifiers};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    ffi::OsString,
    io::{self, Read, Write},
    path::Path,
    sync::{Arc, Mutex, mpsc},
};

struct Screen {
    parser: vt100::Parser,
    eof: bool,
    error: Option<String>,
}

pub struct Terminal {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    input: mpsc::SyncSender<Vec<u8>>,
    screen: Arc<Mutex<Screen>>,
    size: PtySize,
    root: std::path::PathBuf,
    selection: Option<((u16, u16), (u16, u16))>,
}

impl Terminal {
    pub fn start(root: &Path, context: &egui::Context) -> Result<Self, Box<dyn std::error::Error>> {
        Self::spawn(root, context, CommandBuilder::new(shell_name()))
    }

    pub fn spawn(
        root: &Path,
        context: &egui::Context,
        mut shell: CommandBuilder,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let size = PtySize {
            rows: 18,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = native_pty_system().openpty(size)?;
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        shell.cwd(root);
        shell.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(shell)?;
        drop(pair.slave);

        let screen = Arc::new(Mutex::new(Screen {
            parser: vt100::Parser::new(size.rows, size.cols, 1000),
            eof: false,
            error: None,
        }));
        let output = Arc::clone(&screen);
        let repaint = context.clone();
        std::thread::spawn(move || {
            let mut buffer = [0; 8192];
            loop {
                let read = reader.read(&mut buffer);
                let Ok(mut output) = output.lock() else { break };
                match read {
                    Ok(0) => output.eof = true,
                    Ok(bytes) => output.parser.process(&buffer[..bytes]),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => output.eof = true,
                }
                repaint.request_repaint();
                if output.eof {
                    break;
                }
            }
        });
        let (input, commands) = mpsc::sync_channel::<Vec<u8>>(32);
        let output = Arc::clone(&screen);
        let repaint = context.clone();
        std::thread::spawn(move || {
            for command in commands {
                if let Err(error) = writer.write_all(&command).and_then(|_| writer.flush()) {
                    if let Ok(mut output) = output.lock() {
                        output.error = Some(error.to_string());
                    }
                    repaint.request_repaint();
                    break;
                }
            }
        });
        Ok(Self {
            master: pair.master,
            child,
            input,
            screen,
            size,
            root: root.to_path_buf(),
            selection: None,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let font = FontId::monospace(13.0);
        let row_height = ui.fonts(|fonts| fonts.row_height(&font));
        let column_width = ui.fonts(|fonts| fonts.glyph_width(&font, 'M')).max(1.0);
        // Keep the status line inside the allocated panel height.
        let available_height =
            (ui.available_height() - row_height - ui.spacing().item_spacing.y).max(row_height);
        let size = PtySize {
            rows: ((available_height / row_height) as u16).clamp(1, 40),
            cols: ((ui.available_width() / column_width) as u16).clamp(20, 240),
            pixel_width: 0,
            pixel_height: 0,
        };
        if (self.size.rows, self.size.cols) != (size.rows, size.cols) {
            if let Err(error) = self.master.resize(size) {
                ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
            } else if let Ok(mut screen) = self.screen.lock() {
                screen.parser.set_size(size.rows, size.cols);
                self.size = size;
            }
        }

        let Ok(screen) = self.screen.lock() else {
            return;
        };
        let bracketed = screen.parser.screen().bracketed_paste();
        let application_cursor = screen.parser.screen().application_cursor();
        let ended = screen.eof || screen.error.is_some();
        let error = screen.error.clone();
        let snapshot = screen.parser.screen().clone();
        let scroll = screen.parser.screen().scrollback();
        drop(screen);

        let (_, area) = ui.allocate_space(egui::vec2(
            ui.available_width(),
            row_height * self.size.rows as f32,
        ));
        let response = ui.interact(area, input_id(), egui::Sense::click_and_drag());
        if response.clicked() {
            response.request_focus();
        }
        let point = |position: egui::Pos2| {
            let row = ((position.y - area.min.y) / row_height).floor().max(0.0) as u16;
            let col = ((position.x - area.min.x) / column_width).floor().max(0.0) as u16;
            (row.min(size.rows - 1), col.min(size.cols))
        };
        if response.drag_started() {
            response.request_focus();
            if let Some(position) = ui.input(|input| input.pointer.press_origin()) {
                let start = point(position);
                self.selection = Some((start, start));
            }
        }
        if response.dragged() {
            if let (Some((_, end)), Some(position)) =
                (&mut self.selection, response.interact_pointer_pos())
            {
                *end = point(position);
            }
        }
        if response.clicked() {
            self.selection = None;
        }
        let selection = self
            .selection
            .map(|(a, b)| if a <= b { (a, b) } else { (b, a) });
        let selected_text = selection.map(|(a, b)| snapshot.contents_between(a.0, a.1, b.0, b.1));
        if response.has_focus()
            && ui
                .input_mut(|input| input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::C))
        {
            if let Some(text) = &selected_text {
                ui.ctx().copy_text(text.clone());
            }
        }
        response.context_menu(|ui| {
            if ui
                .add_enabled(selected_text.is_some(), egui::Button::new("Copy selection"))
                .clicked()
            {
                ui.ctx()
                    .copy_text(selected_text.clone().unwrap_or_default());
                ui.close_menu();
            }
            if ui.button("Copy visible screen").clicked() {
                ui.ctx().copy_text(snapshot.contents());
                ui.close_menu();
            }
        });
        let painter = ui.painter().with_clip_rect(area);
        for row in 0..size.rows {
            for col in 0..size.cols {
                let Some(cell) = snapshot.cell(row, col) else {
                    continue;
                };
                let rect = egui::Rect::from_min_size(
                    area.min + egui::vec2(col as f32 * column_width, row as f32 * row_height),
                    egui::vec2(
                        column_width * if cell.is_wide() { 2.0 } else { 1.0 },
                        row_height,
                    ),
                );
                let mut foreground = ansi_color(cell.fgcolor(), ui.visuals().text_color());
                let mut background = ansi_color(cell.bgcolor(), ui.visuals().extreme_bg_color);
                if cell.inverse() {
                    std::mem::swap(&mut foreground, &mut background);
                }
                if selection.is_some_and(|(start, end)| start <= (row, col) && (row, col) < end) {
                    background = ui.visuals().selection.bg_fill;
                }
                painter.rect_filled(rect, 0.0, background);
                if !cell.is_wide_continuation() {
                    painter.text(
                        rect.min,
                        egui::Align2::LEFT_TOP,
                        cell.contents(),
                        font.clone(),
                        foreground,
                    );
                }
                if cell.underline() {
                    painter.hline(
                        rect.x_range(),
                        rect.max.y - 1.0,
                        egui::Stroke::new(1.0, foreground),
                    );
                }
            }
        }
        let (cursor_row, cursor_col) = snapshot.cursor_position();
        if response.has_focus() && !snapshot.hide_cursor() && scroll == 0 {
            let position = area.min
                + egui::vec2(
                    cursor_col as f32 * column_width,
                    cursor_row as f32 * row_height,
                );
            painter.rect_stroke(
                egui::Rect::from_min_size(position, egui::vec2(column_width, row_height)),
                0.0,
                egui::Stroke::new(1.0, ui.visuals().text_color()),
                egui::StrokeKind::Inside,
            );
        }
        if response.hovered() {
            let delta = ui.input(|input| input.smooth_scroll_delta.y);
            if delta.abs() >= 1.0 {
                let offset = (delta / row_height).round() as isize;
                if let Ok(mut screen) = self.screen.lock() {
                    screen
                        .parser
                        .set_scrollback(scroll.saturating_add_signed(offset));
                }
            }
        }
        if ended {
            let status = self.child.try_wait().ok().flatten();
            ui.label(error.unwrap_or_else(|| {
                status.map_or_else(
                    || "Process exited.".to_owned(),
                    |status| format!("Process exited with code {}.", status.exit_code()),
                )
            }));
            return;
        }
        if response.has_focus()
            && ui.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::Escape))
        {
            response.surrender_focus();
        }
        if !response.has_focus() {
            ui.add(
                egui::Label::new("Click the terminal to type. Shift+Esc moves focus out.")
                    .truncate(),
            );
            return;
        }

        let mut bytes = Vec::new();
        ui.input_mut(|input| {
            input.events.retain(|event| {
                if let Some(data) = terminal_input(event, bracketed, application_cursor) {
                    bytes.extend_from_slice(&data);
                    false
                } else {
                    true
                }
            });
        });
        if bytes.is_empty() {
            return;
        }
        if bytes.len() > 64 * 1024 {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                "Paste exceeds the 64 KiB input limit.",
            );
        } else if let Err(error) = self.input.try_send(bytes) {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("Terminal input was not sent: {error}"),
            );
        } else if let Ok(mut screen) = self.screen.lock() {
            screen.parser.set_scrollback(0);
        }
    }
}

fn ansi_color(color: vt100::Color, fallback: egui::Color32) -> egui::Color32 {
    use egui::Color32;
    match color {
        vt100::Color::Default => fallback,
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        vt100::Color::Idx(index) => {
            const BASIC: [[u8; 3]; 16] = [
                [30, 30, 36],
                [220, 75, 85],
                [110, 190, 135],
                [220, 185, 100],
                [110, 150, 235],
                [180, 130, 225],
                [100, 190, 205],
                [210, 210, 220],
                [110, 110, 125],
                [255, 125, 135],
                [150, 225, 175],
                [255, 225, 150],
                [150, 190, 255],
                [215, 175, 255],
                [145, 230, 240],
                [250, 250, 255],
            ];
            let [r, g, b] = if index < 16 {
                BASIC[index as usize]
            } else if index < 232 {
                let cube = index - 16;
                let level = |n| if n == 0 { 0 } else { 55 + n * 40 };
                [level(cube / 36), level(cube / 6 % 6), level(cube % 6)]
            } else {
                [8 + (index - 232) * 10; 3]
            };
            Color32::from_rgb(r, g, b)
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn input_id() -> egui::Id {
    egui::Id::new("forgeide-terminal-input")
}

fn shell_name() -> OsString {
    if cfg!(target_os = "windows") {
        std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into())
    } else {
        std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into())
    }
}

fn terminal_input(event: &Event, bracketed: bool, application_cursor: bool) -> Option<Vec<u8>> {
    let sequence: &[u8] = match event {
        Event::Text(text) => return Some(text.as_bytes().to_vec()),
        Event::Paste(text) if bracketed => {
            let mut bytes = b"\x1b[200~".to_vec();
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(b"\x1b[201~");
            return Some(bytes);
        }
        Event::Paste(text) => return Some(text.as_bytes().to_vec()),
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            if modifiers.ctrl {
                return control(*key, *modifiers).map(|byte| vec![byte]);
            }
            match key {
                Key::Enter => b"\r",
                Key::Tab => b"\t",
                Key::Backspace => b"\x7f",
                Key::Escape => b"\x1b",
                Key::ArrowUp if application_cursor => b"\x1bOA",
                Key::ArrowDown if application_cursor => b"\x1bOB",
                Key::ArrowRight if application_cursor => b"\x1bOC",
                Key::ArrowLeft if application_cursor => b"\x1bOD",
                Key::ArrowUp => b"\x1b[A",
                Key::ArrowDown => b"\x1b[B",
                Key::ArrowRight => b"\x1b[C",
                Key::ArrowLeft => b"\x1b[D",
                Key::Delete => b"\x1b[3~",
                Key::Home => b"\x1b[H",
                Key::End => b"\x1b[F",
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(sequence.to_vec())
}

fn control(key: Key, modifiers: Modifiers) -> Option<u8> {
    if modifiers.alt || modifiers.shift || modifiers.mac_cmd {
        return None;
    }
    let symbol = key.name().as_bytes();
    match symbol {
        [letter @ b'A'..=b'Z'] => Some(letter - b'A' + 1),
        _ => None,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn starts_pty_and_captures_real_process_output() -> Result<(), Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let mut shell = CommandBuilder::new("/bin/sh");
        shell.args(["-c", "printf 'ForgeIDE PTY ready\n'"]);
        let terminal = Terminal::spawn(folder.path(), &egui::Context::default(), shell)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if terminal
                .screen
                .lock()
                .unwrap()
                .parser
                .screen()
                .contents()
                .contains("ForgeIDE PTY ready")
            {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Err("PTY did not deliver the child process output".into())
    }
}
