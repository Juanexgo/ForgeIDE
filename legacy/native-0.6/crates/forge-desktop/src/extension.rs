use crate::{
    file::OpenFile,
    language_edit::{FileChange, Preview},
    protocol::{self, Process},
};
use eframe::egui;
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

const EXTENSION_LIMIT: u64 = 256 * 1024;
const TEXT_LIMIT: usize = 8 * 1024;
const FUEL: u64 = 2_000_000;

#[derive(Default)]
pub struct Panel {
    module: Option<PathBuf>,
    package_module: Option<(String, Vec<u8>)>,
    job: Option<Job>,
    notice: String,
    last_error: Option<String>,
}

struct Job {
    process: Process,
    path: PathBuf,
    input: String,
    start: Instant,
    extension: Option<String>,
}

impl Panel {
    pub fn sync_packages(&mut self, library: &crate::packages::Library, root: Option<&Path>) {
        if self
            .package_module
            .as_ref()
            .is_some_and(|(id, _)| !library.is_available_at(id, root))
        {
            self.package_module = None;
        }
        if self
            .job
            .as_ref()
            .and_then(|job| job.extension.as_ref())
            .is_some_and(|id| !library.is_available_at(id, root))
        {
            self.job = None;
            self.notice = "The running extension is disabled or removed.".into();
            self.last_error = Some(self.notice.clone());
        }
    }

    pub fn use_package(&mut self, name: String, module: Vec<u8>) {
        self.module = None;
        self.package_module = Some((name, module));
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path, file: Option<&OpenFile>) {
        ui.label("Choose a local WebAssembly transform. It receives the complete active buffer when you run it.");
        if ui.button("Choose WASM…").clicked() {
            if let Some(module) = rfd::FileDialog::new()
                .add_filter("WebAssembly", &["wasm"])
                .pick_file()
            {
                self.module = Some(module);
                self.package_module = None;
            }
        }
        if self.module.is_some() || self.package_module.is_some() {
            if let Some(module) = &self.module {
                ui.label(module.to_string_lossy());
            }
            if let Some((name, _)) = &self.package_module {
                ui.label(name);
            }
            let eligible =
                self.job.is_none() && file.is_some_and(|file| file.text.len() <= TEXT_LIMIT);
            if ui
                .add_enabled(eligible, egui::Button::new("Run Transform"))
                .clicked()
            {
                let file = file.expect("active file");
                self.start(root, file, ui.ctx());
            }
        }
        if self.job.is_some() && ui.button("Cancel transform").clicked() {
            self.job = None;
            self.notice = "Transform cancelled.".into();
        }
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
    }

    fn start(&mut self, root: &Path, file: &OpenFile, context: &egui::Context) {
        if self.job.is_some() {
            self.notice = "Wait for the running extension or cancel it first.".into();
            self.last_error = Some(self.notice.clone());
            return;
        }
        if file.text.len() > TEXT_LIMIT {
            self.notice = "Extension input exceeds 8 KiB.".into();
            self.last_error = Some(self.notice.clone());
            return;
        }
        let result = (|| -> io::Result<Job> {
            let mut request = if let Some((_, bytes)) = &self.package_module {
                json!({"module_bytes": bytes})
            } else {
                let module = self
                    .module
                    .as_ref()
                    .expect("selected module")
                    .canonicalize()?;
                let module = module
                    .to_str()
                    .ok_or_else(|| io::Error::other("The module path must be valid Unicode."))?;
                json!({"module": module})
            };
            request["input"] = json!(file.text);
            let process = Process::spawn(
                &std::env::current_exe()?,
                &["--extension-worker".into()],
                root,
                context,
            )?;
            process.send(&request)?;
            Ok(Job {
                process,
                input: file.text.clone(),
                path: file.path.clone(),
                start: Instant::now(),
                extension: self.package_module.as_ref().map(|(id, _)| id.clone()),
            })
        })();
        match result {
            Ok(job) => {
                self.job = Some(job);
                self.notice = "Running transform…".into();
            }
            Err(error) => {
                self.notice = error.to_string();
                self.last_error = Some(self.notice.clone());
            }
        }
    }

    pub fn run_command(
        &mut self,
        id: String,
        module: Vec<u8>,
        root: &Path,
        file: &OpenFile,
        context: &egui::Context,
    ) {
        if self.job.is_some() {
            self.notice = "An extension is already running.".into();
            self.last_error = Some(self.notice.clone());
            return;
        }
        self.use_package(id, module);
        self.start(root, file, context);
    }

    pub fn take_error(&mut self) -> Option<String> {
        self.last_error.take()
    }

    pub fn poll(&mut self, context: &egui::Context) -> Option<Preview> {
        let job = self.job.as_ref()?;
        context.request_repaint_after(Duration::from_millis(150));
        let result = match job.process.poll() {
            Ok(Some(result)) => result,
            Ok(None) if job.start.elapsed() < Duration::from_secs(3) => return None,
            Ok(None) => json!({"error": "Extension reached its time limit."}),
            Err(error) => json!({"error": error.to_string()}),
        };
        let job = self.job.take().expect("running job");
        let Some(after) = result["output"].as_str() else {
            self.notice = result["error"]
                .as_str()
                .unwrap_or("Invalid extension response.")
                .to_owned();
            self.last_error = Some(self.notice.clone());
            return None;
        };
        if after == job.input {
            self.notice = "The transform produced no changes.".into();
            return None;
        }
        self.notice = "Review the proposed changes.".into();
        Some(Preview {
            title: "Review extension output",
            files: vec![FileChange {
                path: job.path,
                snippets: vec![(job.input.clone(), after.to_owned())],
                edits: None,
                before: job.input,
                after: after.to_owned(),
            }],
        })
    }
}

pub fn worker() -> io::Result<()> {
    let request = protocol::read(&mut std::io::stdin().lock())?;
    let result = match run_request(&request) {
        Ok(output) => json!({"output": output}),
        Err(error) => json!({"error": error.to_string()}),
    };
    protocol::write(&mut std::io::stdout().lock(), &serde_json::to_vec(&result)?)
}

fn run_request(request: &Value) -> io::Result<String> {
    if let Some(bytes) = request.get("module_bytes") {
        let module: Vec<u8> = serde_json::from_value(bytes.clone())?;
        if module.len() as u64 > EXTENSION_LIMIT {
            return Err(io::Error::other("Extension exceeds 256 KiB."));
        }
        let input = request["input"]
            .as_str()
            .ok_or_else(|| io::Error::other("Missing input."))?;
        return transform(&module, input);
    }
    let path = request["module"]
        .as_str()
        .map(Path::new)
        .ok_or_else(|| io::Error::other("Missing module path."))?;
    let input = request["input"]
        .as_str()
        .ok_or_else(|| io::Error::other("Missing input."))?;
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::other("Select a regular WebAssembly file."));
    }
    let mut module = Vec::new();
    File::open(path)?
        .take(EXTENSION_LIMIT + 1)
        .read_to_end(&mut module)?;
    transform(&module, input)
}

fn transform(wasm: &[u8], input: &str) -> io::Result<String> {
    if wasm.len() as u64 > EXTENSION_LIMIT || input.len() > TEXT_LIMIT {
        return Err(io::Error::other(
            "Extension module or input exceeds its limit.",
        ));
    }

    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm).map_err(io::Error::other)?;
    if module.imports().next().is_some() {
        return Err(io::Error::other("Extension imports are disabled."));
    }

    let limits = StoreLimitsBuilder::new()
        .memory_size(2 * 1024 * 1024)
        .table_elements(1024)
        .instances(1)
        .tables(1)
        .memories(1)
        .build();
    let mut store = Store::new(&engine, limits);
    store.limiter(|limits| limits);
    store.set_fuel(FUEL).map_err(io::Error::other)?;
    let linker = Linker::<StoreLimits>::new(&engine);
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(io::Error::other)?
        .start(&mut store)
        .map_err(io::Error::other)?;
    let memory = instance
        .get_memory(&store, "memory")
        .ok_or_else(|| io::Error::other("Extension must export memory."))?;
    let buffer = instance
        .get_typed_func::<i32, i32>(&store, "forge_alloc")
        .map_err(io::Error::other)?;
    let run = instance
        .get_typed_func::<(i32, i32), i64>(&store, "forge_transform")
        .map_err(io::Error::other)?;
    let ptr = buffer
        .call(&mut store, input.len() as i32)
        .map_err(io::Error::other)?;
    memory
        .write(&mut store, ptr as u32 as usize, input.as_bytes())
        .map_err(io::Error::other)?;
    let result = run
        .call(&mut store, (ptr, input.len() as i32))
        .map_err(io::Error::other)? as u64;
    let ptr = (result >> 32) as usize;
    let len = (result & u64::from(u32::MAX)) as usize;
    if len > TEXT_LIMIT {
        return Err(io::Error::other("Extension output exceeds its limit."));
    }
    let mut bytes = vec![0; len];
    memory
        .read(&store, ptr, &mut bytes)
        .map_err(io::Error::other)?;
    let output = String::from_utf8(bytes).map_err(io::Error::other)?;
    if output.contains('\0') {
        return Err(io::Error::other("Extension output contains binary data."));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_bytes_without_imports() {
        let wasm = wat::parse_str(include_str!(
            "../../../fixtures/extensions/uppercase-first.wat"
        ))
        .unwrap();
        assert_eq!(transform(&wasm, "abc 🦀").unwrap(), "Abc 🦀");
        assert_eq!(transform(&wasm, "").unwrap(), "");
    }

    #[test]
    fn rejects_imports_and_memory_over_budget_and_interrupts_a_loop() {
        let imported =
            wat::parse_str(r#"(module (import "wasi_snapshot_preview1" "fd_write" (func)))"#)
                .unwrap();
        assert!(
            transform(&imported, "")
                .unwrap_err()
                .to_string()
                .contains("imports")
        );
        let oversized = wat::parse_str(r#"(module (memory (export "memory") 33))"#).unwrap();
        assert!(transform(&oversized, "").is_err());
        let loop_forever = wat::parse_str(
            r#"(module
            (memory (export "memory") 1)
            (func (export "forge_alloc") (param i32) (result i32) (i32.const 0))
            (func (export "forge_transform") (param i32 i32) (result i64)
                (loop $forever (br $forever))
                (i64.const 0)))"#,
        )
        .unwrap();
        assert!(
            transform(&loop_forever, "")
                .unwrap_err()
                .to_string()
                .contains("fuel")
        );
    }
}
