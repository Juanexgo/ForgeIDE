use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
};

#[test]
fn packaged_entry_point_runs_an_isolated_extension() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let module = dir.path().join("transform.wasm");
    let bytes = wat::parse_str(include_str!(
        "../../../fixtures/extensions/uppercase-first.wat"
    ))?;
    std::fs::write(&module, &bytes)?;

    for request in [
        json!({"module": module, "input": "hello 🦀"}),
        json!({"module_bytes": bytes, "input": "hello 🦀"}),
    ] {
        let mut worker = Command::new(env!("CARGO_BIN_EXE_forgeide"))
            .arg("--extension-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut input = worker.stdin.take().expect("piped stdin");
        let message = serde_json::to_vec(&request)?;
        write!(input, "Content-Length: {}\r\n\r\n", message.len())?;
        input.write_all(&message)?;
        drop(input);

        let mut output = BufReader::new(worker.stdout.take().expect("piped stdout"));
        let mut header = String::new();
        output.read_line(&mut header)?;
        let bytes: u64 = header
            .strip_prefix("Content-Length: ")
            .expect("message length")
            .trim()
            .parse()?;
        assert!(bytes < 1024);
        let mut separator = String::new();
        output.read_line(&mut separator)?;
        assert_eq!(separator, "\r\n");
        let response: Value = serde_json::from_reader(output.take(bytes))?;
        assert_eq!(response["output"], "Hello 🦀");
        assert!(worker.wait()?.success());
    }
    Ok(())
}

#[test]
fn sdk_packages_and_inspects_portable_commands_without_starting_the_editor()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let module = directory.path().join("uppercase-first.wasm");
    std::fs::write(
        &module,
        wat::parse_str(include_str!(
            "../../../fixtures/extensions/uppercase-first.wat"
        ))?,
    )?;
    let manifest = directory.path().join("forge-extension.json");
    std::fs::write(
        &manifest,
        include_str!("../../../fixtures/extensions/toolbox/forge-extension.json"),
    )?;
    let package = directory.path().join("toolbox.forge-extension");
    let binary = env!("CARGO_BIN_EXE_forgeide");
    let result = Command::new(binary)
        .arg("--extension-package")
        .arg(&manifest)
        .arg(&package)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::remove_file(&module)?;
    std::fs::remove_file(&manifest)?;
    let report = Command::new(binary)
        .arg("--extension-inspect")
        .arg(&package)
        .output()?;
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let value: Value = serde_json::from_slice(&report.stdout)?;
    assert_eq!(value["id"], "forge.toolbox");
    assert_eq!(value["commands"][0]["id"], "uppercase-first");
    assert_eq!(value["snippets"], 1);
    assert_eq!(
        value["permissions"][0],
        "active-document-on-explicit-command"
    );
    assert_eq!(value["javascript_required"], false);
    std::fs::write(&package, b"not a package")?;
    let result = Command::new(binary)
        .arg("--extension-inspect")
        .arg(&package)
        .output()?;
    assert!(!result.status.success());
    Ok(())
}
