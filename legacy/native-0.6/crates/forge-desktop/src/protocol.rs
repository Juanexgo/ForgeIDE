use eframe::egui;
use serde_json::Value;
use std::{
    ffi::OsString,
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
};

const HEADER_LIMIT: u64 = 8192;
const BODY_LIMIT: usize = 16 * 1024 * 1024;

pub struct Process {
    child: Child,
    inbound: Receiver<io::Result<Value>>,
    outbound: SyncSender<Vec<u8>>,
}

impl Process {
    pub fn spawn(
        executable: &Path,
        args: &[OsString],
        root: &Path,
        context: &egui::Context,
    ) -> io::Result<Self> {
        executable.canonicalize()?;
        // Preserve the invocation name for rustup, node and virtual-environment shims.
        let executable = std::path::absolute(executable)?;
        let mut child = Command::new(executable)
            .args(args)
            .current_dir(root)
            .env("PATH", crate::project::environment_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut output = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut input = child.stdin.take().expect("piped stdin");
        let (reader, inbound) = mpsc::sync_channel(8);
        let (outbound, writer) = mpsc::sync_channel::<Vec<u8>>(8);
        let repaint = context.clone();
        let error_channel = reader.clone();
        std::thread::spawn(move || {
            loop {
                let result = read(&mut output);
                let failed = result.is_err();
                if reader.send(result).is_err() {
                    break;
                }
                repaint.request_repaint();
                if failed {
                    break;
                }
            }
        });
        let repaint = context.clone();
        std::thread::spawn(move || {
            for message in writer {
                let result = write(&mut input, &message);
                if let Err(error) = result {
                    let _ = error_channel.send(Err(error));
                    repaint.request_repaint();
                    break;
                }
            }
        });
        Ok(Self {
            child,
            inbound,
            outbound,
        })
    }

    pub fn send(&self, value: &Value) -> io::Result<()> {
        let bytes = serde_json::to_vec(value)?;
        if bytes.len() > BODY_LIMIT {
            return Err(io::Error::other("Protocol message exceeds 16 MiB."));
        }
        self.outbound.try_send(bytes).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => io::Error::other("Service message queue is full."),
            mpsc::TrySendError::Disconnected(_) => io::Error::other("Service input closed."),
        })
    }

    pub fn poll(&self) -> io::Result<Option<Value>> {
        match self.inbound.try_recv() {
            Ok(result) => result.map(Some),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(io::Error::other("Service process exited."))
            }
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn write(writer: &mut impl Write, message: &[u8]) -> io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n", message.len())?;
    writer.write_all(message)?;
    writer.flush()
}

pub fn read(reader: &mut impl BufRead) -> io::Result<Value> {
    let mut header = Vec::new();
    let mut remaining = HEADER_LIMIT;
    let mut length = None;
    loop {
        header.clear();
        let count = reader.take(remaining).read_until(b'\n', &mut header)?;
        if count == 0 || !header.ends_with(b"\r\n") {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Invalid or truncated service header.",
            ));
        }
        if header == b"\r\n" {
            break;
        }
        remaining -= count as u64;
        let field = std::str::from_utf8(&header[..header.len() - 2]).map_err(io::Error::other)?;
        let Some((key, value)) = field.split_once(':') else {
            return Err(io::Error::other("Malformed service header field."));
        };
        if key.eq_ignore_ascii_case("Content-Length") {
            if length.is_some() {
                return Err(io::Error::other("Duplicate Content-Length."));
            }
            length = Some(value.trim().parse::<usize>().map_err(io::Error::other)?);
        }
    }
    let length = length
        .filter(|value| *value <= BODY_LIMIT)
        .ok_or_else(|| io::Error::other("Invalid service body length."))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_consecutive_frames_and_bounds_lengths() {
        let frames = b"Content-Length: 7\r\n\r\n{\"x\":1}content-length: 2\r\n\r\n[]";
        let mut reader = &frames[..];
        assert_eq!(read(&mut reader).unwrap()["x"], 1);
        assert!(read(&mut reader).unwrap().is_array());
        for invalid in [
            &b"Content-Length: 99999999999\r\n\r\n"[..],
            &b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}"[..],
            &b"Content-Length: 2\r\n\r\n{"[..],
        ] {
            assert!(read(&mut &invalid[..]).is_err());
        }
    }
}

#[cfg(all(test, unix))]
mod shim_tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn service_process_uses_the_selected_shim_invocation_name() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("dispatcher");
        let shim = directory.path().join("shim-server");
        let script = "#!/bin/sh\ncase \"$0\" in\n*shim-server) printf 'Content-Length: 13\\r\\n\\r\\n{\"shim\":true}';;\n*) printf 'Content-Length: 14\\r\\n\\r\\n{\"shim\":false}';;\nesac\n";
        std::fs::write(&target, script)?;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700))?;
        symlink(&target, &shim)?;
        let process = Process::spawn(&shim, &[], directory.path(), &egui::Context::default())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if let Some(message) = process.poll()? {
                assert_eq!(message["shim"], true);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(())
    }
}
