use eframe::egui;
use serde_json::Value;
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

static USE_GITHUB_CREDENTIALS: AtomicBool = AtomicBool::new(false);

pub(crate) fn credential_helper(root: &Path) -> Option<PathBuf> {
    USE_GITHUB_CREDENTIALS
        .load(Ordering::Relaxed)
        .then(|| locate_gh(root))
        .flatten()
}

fn remember_connection(connected: bool) -> io::Result<()> {
    USE_GITHUB_CREDENTIALS.store(connected, Ordering::Relaxed);
    crate::packages::atomic_write(
        &directory()?.join("connection.state"),
        if connected { b"true" } else { b"false" },
    )
}

#[derive(Clone, Debug, Default)]
pub struct Identity {
    pub login: String,
    pub state: String,
    pub source: String,
    pub active: bool,
}
#[derive(Default)]
pub struct Panel {
    identities: Vec<Identity>,
    job: Option<mpsc::Receiver<io::Result<Vec<Identity>>>>,
    login: Option<Login>,
    notice: String,
    checked: bool,
    pending_logout: Option<String>,
}
struct Login {
    child: Child,
    output: mpsc::Receiver<String>,
    text: String,
    started: Instant,
}
impl Drop for Login {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn locate_gh(root: &Path) -> Option<PathBuf> {
    let executable = if cfg!(windows) { "gh.exe" } else { "gh" };
    let mut directories: Vec<_> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    if cfg!(target_os = "macos") {
        directories.extend(
            ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
                .into_iter()
                .map(PathBuf::from),
        );
    }
    if cfg!(windows) {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            directories.push(PathBuf::from(program_files).join("GitHub CLI"));
        }
    }
    directories
        .into_iter()
        .filter(|directory| directory.is_absolute())
        .filter_map(|directory| directory.join(executable).canonicalize().ok())
        .find(|path| path.is_file() && !path.starts_with(root))
}

pub fn redact(text: &str) -> String {
    static TOKENS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static URLS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let tokens = TOKENS.get_or_init(|| {
        regex::Regex::new(r"(?:gh[pousr]_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+)")
            .expect("token pattern")
    });
    let urls =
        URLS.get_or_init(|| regex::Regex::new(r"(?i)(https?://)[^/\s@]+@").expect("URL pattern"));
    urls.replace_all(&tokens.replace_all(text, "[redacted]"), "$1[redacted]@")
        .into_owned()
}

fn directory() -> io::Result<PathBuf> {
    let root = crate::recovery::directory()?
        .parent()
        .expect("app data directory")
        .join("Accounts");
    std::fs::create_dir_all(&root)?;
    Ok(root)
}
fn gh_command(executable: &Path, cwd: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .env("PATH", crate::project::environment_path())
        .current_dir(cwd)
        .env("NO_COLOR", "1")
        .env("GH_PAGER", "cat")
        .env("GH_PROMPT_DISABLED", "1");
    // These overrides can expose credentials or change the repository being targeted.
    command
        .env_remove("GH_REPO")
        .env_remove("GH_DEBUG")
        .env_remove("DEBUG");
    command
}
pub(crate) fn run_cli(executable: &Path, cwd: &Path, args: &[&str]) -> io::Result<Vec<u8>> {
    let mut child = gh_command(executable, cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = crate::git::collect(child.stdout.take().expect("stdout"));
    let stderr = crate::git::collect(child.stderr.take().expect("stderr"));
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        if let Some(exit) = child.try_wait()? {
            let bytes = crate::git::read_result(stdout, deadline)?;
            let errors = crate::git::read_result(stderr, deadline)?;
            if !exit.success() {
                return Err(io::Error::other(redact(&String::from_utf8_lossy(&errors))));
            }
            return Ok(bytes);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(
                "GitHub account operation timed out. Try again.",
            ));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}
fn status(executable: &Path, cwd: &Path) -> io::Result<Vec<Identity>> {
    parse_identity(&serde_json::from_slice(&run_cli(
        executable,
        cwd,
        &[
            "auth",
            "status",
            "--hostname",
            "github.com",
            "--json",
            "hosts",
        ],
    )?)?)
}
fn parse_identity(value: &Value) -> io::Result<Vec<Identity>> {
    let hosts = value["hosts"].as_object().ok_or_else(|| {
        io::Error::other("Update GitHub CLI: account status JSON was not recognized.")
    })?;
    Ok(hosts
        .get("github.com")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let login = item["login"].as_str()?;
            Some(Identity {
                login: login.chars().take(100).collect(),
                state: item["state"]
                    .as_str()
                    .unwrap_or("unknown")
                    .chars()
                    .take(100)
                    .collect(),
                active: item["active"].as_bool().unwrap_or(false),
                source: item["tokenSource"]
                    .as_str()
                    .unwrap_or("configured credential store")
                    .chars()
                    .take(200)
                    .collect(),
            })
        })
        .collect())
}

impl Panel {
    pub fn load() -> Self {
        let connected = directory()
            .ok()
            .and_then(|root| crate::packages::read_bounded(&root.join("connection.state"), 16).ok())
            .is_some_and(|bytes| bytes == b"true");
        USE_GITHUB_CREDENTIALS.store(connected, Ordering::Relaxed);
        Self::default()
    }

    pub fn tick(&mut self, context: &egui::Context) {
        if let Some(login) = &mut self.login {
            for chunk in login.output.try_iter() {
                let room = (32 * 1024usize).saturating_sub(login.text.len());
                if room > 0 {
                    login.text.extend(chunk.chars().take(room));
                }
            }
            let result = login.child.try_wait();
            match result {
                Ok(Some(exit)) => {
                    let text = login.text.clone();
                    self.login = None;
                    self.notice = if exit.success() {
                        "GitHub authorization finished. Checking the account…".into()
                    } else {
                        format!("Sign-in did not finish. {}", redact(&text))
                    };
                    if exit.success() {
                        self.check(context);
                    }
                }
                Err(error) => {
                    self.notice = error.to_string();
                    self.login = None;
                }
                Ok(None) if login.started.elapsed() > Duration::from_secs(15 * 60) => {
                    self.login = None;
                    self.notice = "The sign-in session expired. Start again.".into();
                }
                _ => {}
            }
        }
        if let Some(job) = &self.job {
            match job.try_recv() {
                Ok(result) => {
                    self.job = None;
                    match result {
                        Ok(identities) => {
                            self.identities = identities;
                            let connected = self
                                .identities
                                .iter()
                                .any(|identity| identity.state == "success" && identity.active);
                            if let Err(error) = remember_connection(connected) {
                                self.notice = format!(
                                    "Account checked, but preference could not be saved: {error}"
                                );
                            }

                            self.notice = if self.identities.is_empty() {
                                "Not signed in to GitHub.".into()
                            } else {
                                "Account checked.".into()
                            };
                        }
                        Err(error) => self.notice = redact(&error.to_string()),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.job = None;
                    self.notice = "Account check stopped unexpectedly.".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.job.is_some() || self.login.is_some() {
            context.request_repaint_after(Duration::from_millis(120));
        }
    }

    pub fn check(&mut self, context: &egui::Context) {
        if self.job.is_some() || self.login.is_some() {
            return;
        }
        self.checked = true;
        let result = directory().and_then(|root| {
            locate_gh(&root)
                .map(|gh| (root, gh))
                .ok_or_else(|| io::Error::other("Install GitHub CLI (gh) to connect an account."))
        });
        let (root, gh) = match result {
            Ok(pair) => pair,
            Err(error) => {
                self.notice = error.to_string();
                return;
            }
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        self.job = Some(receiver);
        let context = context.clone();
        std::thread::spawn(move || {
            let _ = sender.send(status(&gh, &root));
            context.request_repaint();
        });
    }

    fn change_account(&mut self, context: &egui::Context, login: String, logout: bool) {
        if self.job.is_some() || self.login.is_some() {
            return;
        }
        if login.is_empty()
            || login.len() > 100
            || !login
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            self.notice = "Invalid GitHub login.".into();
            return;
        }
        let result = directory().and_then(|root| {
            locate_gh(&root)
                .map(|gh| (root, gh))
                .ok_or_else(|| io::Error::other("GitHub CLI was not found."))
        });
        let (root, gh) = match result {
            Ok(pair) => pair,
            Err(error) => {
                self.notice = error.to_string();
                return;
            }
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let context = context.clone();
        self.job = Some(receiver);
        std::thread::spawn(move || {
            let operation = if logout { "logout" } else { "switch" };
            let result = run_cli(
                &gh,
                &root,
                &[
                    "auth",
                    operation,
                    "--hostname",
                    "github.com",
                    "--user",
                    &login,
                ],
            )
            .and_then(|_| status(&gh, &root));
            let _ = sender.send(result);
            context.request_repaint();
        });
    }

    fn sign_in(&mut self) -> io::Result<()> {
        if self.job.is_some() || self.login.is_some() {
            return Err(io::Error::other("An account operation is already running."));
        }
        let root = directory()?;
        let gh = locate_gh(&root)
            .ok_or_else(|| io::Error::other("Install GitHub CLI (gh), then retry."))?;
        let mut child = gh_command(&gh, &root)
            .args([
                "auth",
                "login",
                "--hostname",
                "github.com",
                "--web",
                "--git-protocol",
                "https",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(mut input) = child.stdin.take() {
            let _ = input.write_all(b"\n");
        }
        let (sender, receiver) = mpsc::channel();
        for mut stream in [
            Box::new(child.stdout.take().expect("stdout")) as Box<dyn Read + Send>,
            Box::new(child.stderr.take().expect("stderr")),
        ] {
            let sender = sender.clone();
            std::thread::spawn(move || {
                let mut buffer = [0u8; 2048];
                let mut total = 0usize;
                loop {
                    let count = match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => count,
                    };
                    total += count;
                    if total <= 32 * 1024
                        && sender
                            .send(redact(&String::from_utf8_lossy(&buffer[..count])))
                            .is_err()
                    {
                        break;
                    }
                }
            });
        }
        self.login = Some(Login {
            child,
            output: receiver,
            text: String::new(),
            started: Instant::now(),
        });
        self.notice =
            "Authorize GitHub CLI in your browser. ForgeIDE will detect completion.".into();
        Ok(())
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.tick(ui.ctx());
        ui.strong("GitHub account");
        if !self.checked && self.job.is_none() {
            self.check(ui.ctx());
        }
        let busy = self.job.is_some() || self.login.is_some();
        for identity in self.identities.clone() {
            ui.label(format!(
                "@{} · {}{}",
                identity.login,
                identity.state,
                if identity.active { " · active" } else { "" }
            ));
            ui.label(crate::chrome::small(format!(
                "Credentials — {}",
                identity.source
            )));
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !busy && !identity.active,
                        egui::Button::new("Use this account").small(),
                    )
                    .clicked()
                {
                    self.change_account(ui.ctx(), identity.login.clone(), false);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Sign out…").small())
                    .clicked()
                {
                    self.pending_logout = Some(identity.login.clone());
                }
            });
        }
        if let Some(login) = self.pending_logout.clone() {
            egui::Frame::group(ui.style()).show(ui,|ui| {
                ui.label(format!("Sign @{login} out of GitHub CLI? This also affects other tools using that CLI account."));
                ui.horizontal(|ui| {
                    if ui.add_enabled(!busy,egui::Button::new("Sign out account")).clicked() { self.pending_logout = None; self.change_account(ui.ctx(),login,true); }
                    if ui.button("Keep account").clicked() { self.pending_logout = None; }
                });
            });
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Check account"))
                .clicked()
            {
                self.check(ui.ctx());
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Sign in with GitHub…"))
                .clicked()
            {
                if let Err(error) = self.sign_in() {
                    self.notice = error.to_string();
                }
            }
        });
        if !self.checked && self.identities.is_empty() {
            ui.label(
                "Git works locally without an account. Connect GitHub for private HTTPS remotes.",
            );
        }
        if let Some(login) = &self.login {
            ui.spinner();
            ui.hyperlink_to(
                "Open GitHub device authorization",
                "https://github.com/login/device",
            );
            ui.add(egui::Label::new(redact(&login.text)).selectable(true));
            if ui.button("Cancel sign-in").clicked() {
                self.login = None;
                self.notice =
                    "Sign-in cancelled locally. A completed browser grant is managed in GitHub."
                        .into();
            }
        }
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        ui.collapsing("About account access",|ui| {
            ui.label("Authorization happens on GitHub, not in an embedded password form. GitHub CLI manages credentials using its system credential configuration. ForgeIDE does not copy tokens into project files. CLI may fall back to its configuration file when a secure store is unavailable.");
            ui.hyperlink_to("GitHub CLI setup","https://cli.github.com/");
            ui.hyperlink_to("Manage authorized applications","https://github.com/settings/applications");
        });
    }
}

pub(crate) fn validate_repository_name(name: &str) -> io::Result<()> {
    let (owner, repository) = name
        .split_once('/')
        .ok_or_else(|| io::Error::other("Use owner/repository."))?;
    if owner.is_empty()
        || owner.len() > 39
        || owner.starts_with('-')
        || owner.ends_with('-')
        || !owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || repository.is_empty()
        || repository.len() > 100
        || repository.starts_with('-')
        || matches!(repository, "." | "..")
        || !repository
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(io::Error::other(
            "Enter a valid GitHub owner/repository name.",
        ));
    }
    Ok(())
}

pub(crate) fn create_repository(root: &Path, name: &str, private: bool) -> io::Result<()> {
    validate_repository_name(name)?;
    let gh = locate_gh(root)
        .ok_or_else(|| io::Error::other("Install GitHub CLI and sign in before publishing."))?;
    let source = root
        .to_str()
        .ok_or_else(|| io::Error::other("The repository path must be Unicode."))?;
    run_cli(
        &gh,
        root,
        &[
            "repo",
            "create",
            name,
            if private { "--private" } else { "--public" },
            "--source",
            source,
            "--remote",
            "origin",
        ],
    )?;
    let _ = remember_connection(true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_status_keeps_only_display_fields_and_redacts_secrets() {
        let identities = parse_identity(&serde_json::json!({"hosts":{"github.com":[{"login":"octocat","state":"success","tokenSource":"keyring","oauth_token":"must-not-be-retained"}]}})).unwrap();
        assert_eq!(identities[0].login, "octocat");
        assert_eq!(identities[0].state, "success");
        assert_eq!(identities[0].source, "keyring");
        assert!(!identities[0].active);
        assert!(validate_repository_name("octocat/project").is_ok());
        for value in [
            "--public/repo",
            "octocat/../escape",
            "octocat/.",
            "octocat/--flag",
            "missing-owner",
        ] {
            assert!(validate_repository_name(value).is_err());
        }
        let result = redact("ghp_abcdef https://user:secret@github.com/repo github_pat_abc123");
        assert!(
            !result.contains("abcdef") && !result.contains("secret") && !result.contains("abc123")
        );
    }
}
