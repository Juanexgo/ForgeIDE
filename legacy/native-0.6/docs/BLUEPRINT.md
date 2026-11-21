# ForgeIDE technical blueprint

This document specifies the 1.0 target. See [verification and outstanding gates](VERIFICATION.md) for the current implementation status.

## Product requirements

ForgeIDE lets developers open an existing local project and complete a change from navigation through debugging and commit in a native desktop app. The initial audience is individual developers and small teams working with Rust and TypeScript/JavaScript. Require representative repositories from both ecosystems during beta. Files remain ordinary source files, configuration is inspectable, and installed toolchains work offline.

| Workflow | 1.0 requirements |
|---|---|
| Open and resume | Open a local folder or file from GUI or CLI, restore tabs and split views, browse the project tree, search files and content with ignore support, and recover unsaved buffers after abnormal exit. |
| Edit | Keyboard-driven palette and keymaps, multiple selections, grouped undo/redo, find/replace, syntax highlighting and folding, indentation, bracket matching, system clipboard, IME, mixed-direction text, and screen-reader navigation. Preserve UTF‑8, BOM and existing line endings. Route binary or unsupported encodings to an explicit read-only view. |
| Understand | Rust Analyzer and TypeScript language service profiles. Completion, hover, diagnostics, go-to-definition, references, rename, code actions and formatting are capability-gated and version-checked. Preview workspace-wide edits. |
| Run and debug | Integrated terminal and user-approved tasks. Ship tested Rust and Node debug profiles with launch, local attach, breakpoints, step, continue, stack, variables and clean shutdown. Provide actionable missing-tool errors. |
| Version | Git status, staged and unstaged diffs, stage/unstage files, commit and identify merge conflicts. Use terminal for advanced history operations in 1.0. |
| Extend | Install supported language packs, commands, snippets and themes through scoped host capabilities. Inspect permissions, disable or uninstall extensions, and start in safe mode. |
| Optional AI | Preview named files or selections before transmission. Offer reviewable patch suggestions behind an opt-in feature. AI may ship after 1.0. |

Initial release candidates target macOS 14+ arm64, Windows 11 x64 and Ubuntu 24.04 x64 on Wayland and X11. Advertise additional architectures and distributions only when the same acceptance suite runs on them. Local single-root workspaces are the 1.0 support baseline. Remote execution, collaboration, VS Code extension compatibility, notebook runtimes and mobile/web editions are future work. Product success is measured by task completion, performance traces and recovery tests from consenting beta users. Telemetry is opt-in.

## Differentiation to validate

| Reference product | ForgeIDE opportunity | Validation |
|---|---|---|
| VS Code has mature remote workflows and Workspace Trust. | Make local development predictable with a per-workspace view of active tools, exact granted permissions and restart status. Keep extension permissions enforceable through brokered host calls. | Open an unfamiliar repository in restricted mode and inspect whether each background process has a user-visible reason to run. |
| Cursor makes agent workflows central and offers privacy controls. | Make the full edit, debug and commit loop available offline. If AI is enabled, show the outbound context and give the user control over applying patches. | Complete both language workflows with the network blocked, then inspect a captured opt-in request against the context shown in the UI. |
| Zed already ships a native experience on the three OSes and supports Wasm extensions. | Focus on recovery, service isolation and consistent, documented debugger profiles. Publish per-OS test runs for recovery and workflow parity. | Kill the editor while editing and kill each external service independently. Verify recovery, correct status and completion of the supported workflow. |

These are product hypotheses. Measure comparable tasks on the same hardware during beta before making speed or reliability claims. [VS Code Remote Development](https://code.visualstudio.com/docs/remote/remote-overview), [Workspace Trust](https://code.visualstudio.com/docs/editing/workspaces/workspace-trust), [Cursor Agent](https://cursor.com/help/ai-features/agent), [Cursor privacy](https://prod.cursor.com/help/security-and-privacy/privacy), [Zed installation](https://zed.dev/docs/installation), [Zed extensions](https://zed.dev/docs/extensions/developing-extensions).

## Architecture decisions

- **Rust workspace.** Desktop process owns the UI event loop. A per-workspace coordinator serializes changes and publishes revisioned events. CPU-intensive index, search, syntax and diff jobs use bounded workers. Tokio supervises I/O and service lifecycles. Workers consume snapshots, carry request IDs and cancellation tokens, and return results tagged by document revision or service generation. The UI merges coalescible updates and drops obsolete results. Back pressure never silently drops user edits.
- **Text.** Ropey behind `forge-text`; copy-on-write snapshots and predictable insertion in large files. A flat string makes repeated middle edits expensive. A piece table offers cheap persistent edit history but brings its own indexing and compaction complexity. Keep history as bounded transactions with inverses and selection state. UTF-8 bytes for core ranges, grapheme clusters for cursor movement, standard bidi/shaping for display, and explicit per-protocol codecs. Preserve original bytes on save for valid UTF-8, with explicit newline policy for inserted lines. Adopt a documented size ceiling with read-only large-file mode; long-line bounds are independent of file size. [Ropey](https://github.com/cessen/ropey)
- **Desktop rendering.** Evaluate GPUI first using a pinned revision behind desktop-owned view types. The spike must prove scroll/selection latency, font fallback, bidirectional hit testing, IME composition, clipboard, multiple displays, fractional DPI, screen-reader caret and selection, and clean window startup on each target. Record rendering backend and driver coverage. A shared Qt 6 Widgets shell via a narrow C++/Rust bridge is the fallback if GPUI cannot close accessibility or platform gaps during M1. Qt adds deployment and licensing work. Custom winit/wgpu would maximize control and also require building more text, accessibility and desktop plumbing. No renderer types cross into the core. [Zed platform implementation notes](https://zed.dev/docs/development)
- **Syntax.** Tree-sitter with curated, version-pinned grammars and query bundles. Worker converts ordered core edits into incremental tree edits in the order actually applied, reuses the old tree, and uses a full parse if the chain breaks. Snapshot callbacks serve chunks. Publish only matching revisions and bound work on large files. Track ABI and grammar/query compatibility. Highlight visible ranges first and use plain text on timeout. [Tree-sitter incremental parsing](https://tree-sitter.github.io/tree-sitter/using-parsers/3-advanced-parsing.html)
- **Language services.** Supervised local stdio processes. Use LSP 3.17 as the compatibility baseline and negotiate capabilities and position encoding, defaulting to UTF-16 when unspecified. Respect full versus incremental sync. Versions are monotonic per open document and server session, with explicit mapping to core revisions. Follow initialize/initialized and shutdown/exit handshakes with deadlines; restart with backoff and resend open snapshots. Reject stale versioned edits and re-request or mark unversioned diagnostics potentially stale. Prefer project-local tools only through explicit trust and executable selection. A pinned pack can retrieve verified binaries on explicit download. [LSP](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
- **Debugging.** DAP distinct from LSP transport semantics. Supervise adapter, debuggee and integrated terminal lifetimes separately. Support explicit initialize, launch/attach, initialized, setBreakpoints, configurationDone, stopped and terminated transitions as capabilities allow. Track pending and verified breakpoints, adapter index bases and path mappings. Drop old variable handles after resume. Evaluate only when requested. Spike packaged LLDB DAP and a supported Node adapter on the entire matrix, including distribution rights, launch and attach. [DAP](https://microsoft.github.io/debug-adapter-protocol/)
- **Terminal and tasks.** Platform PTY through a narrow adapter, initially portable-pty with POSIX PTY and Windows ConPTY. Evaluate a reusable VT engine such as alacritty_terminal against resize, Unicode, alternate screen and scrollback tests. Use bounded scrollback, ordered reliable I/O and explicit exit/error reporting. Allow terminal hyperlinks only through user action and clipboard writes through policy. Spawn tasks via argv and a chosen working directory. Explicit shell tasks and interactive shell sessions execute with normal user authority. Use Unix process groups and Windows Job Objects where supported and test clean close.
- **Git.** Use a vetted user-installed Git via argv and structured output such as porcelain v2 with NUL delimiters. This preserves user Git semantics and credential helpers with a smaller surface than implementing Git. It also brings an external dependency and configuration side effects. Treat Git as executable code. In restricted folders, show filesystem-only explorer data. In trusted folders, disable automatic hooks, external diff and textconv in background paths and sanitize environment/configuration as applicable. Enable user-authorized operations that invoke hooks or helpers only through the explicit command surface. Serialize index writes per repository and detect external conflicts.
- **Extensions.** Wasmtime component model and versioned WIT with a small host import surface. Separate extension-host process; authorize each call in the desktop broker. No inherited WASI directories, sockets, environment or subprocess access by default. Start with commands, theme and snippet contributions, selected text reads and reviewed edit proposals. Language server launches are separate trusted tool processes under workspace policy. Use fuel, epoch interruption, memory caps and host-call quotas, with an independent watchdog and process teardown. Manifest declares publisher, package and SDK version, contribution IDs and requested capabilities. Verify signed manifests and content digests, then review new grants on upgrade and support revocation and safe-mode startup. [Wasmtime security](https://docs.wasmtime.dev/security.html)
- **Optional AI.** Provider trait behind a separate capability. Explicit context selection with exclusion rules and egress controls. Use OS credential storage. Local providers and remote providers have distinct consent and settings. Stream output with cancellation, validate returned patches against the source revision, and route modifications through the same diff approval as other workspace edits. AI output has no execution authority.

## Planned repository layout

```text
forgeide/
  Cargo.toml                   # workspace and shared dependencies
  crates/
    forge-text/                # implemented buffer and snapshots
    forge-workspace/           # documents, VFS, watchers and transactions
    forge-protocol/            # IDs, errors, worker IPC envelopes
    forge-storage/             # recovery, config, session schema
    forge-syntax/              # grammar/query catalog and parse jobs
    forge-process/             # trusted launch and lifecycle
    forge-lsp/                 # LSP translation and lifecycle
    forge-dap/                 # DAP sessions and debugger profiles
    forge-terminal/            # PTY and VT engine
    forge-git/                 # status, diff, staging, commit
    forge-extension-host/      # isolated component host binary
    forge-desktop/             # UI, commands, accessibility, app main
    forge-ai/                  # optional, late-bound provider interface
  sdk/wit/                     # extension schema; generate bindings
  packs/                       # pinned language and debugger recipes
  fixtures/                    # synthetic projects and protocol transcripts
  packaging/                   # per-platform installers and verification
  docs/adr/                    # decisions with evidence and triggers to revisit
  .github/workflows/           # CI, installer verification, release promotion
```

Add each crate when it has a consumer. Core crates depend on neither desktop nor extension-host. Desktop orchestrates service crates through typed interfaces. Persistent state is per-user, using OS app-data locations. Versioned project configuration declares preferences and proposed tasks; trusted decisions and credentials live outside the project. Use SQLite for session/workspace metadata with explicit schema migrations, and separate bounded recovery records for document snapshots and deltas. Derived search indexes may be discarded and rebuilt.

## Contracts and data integrity

| Contract | Semantics |
|---|---|
| `Document::apply(expected, edits) -> Result<Revision, EditError>` | Implemented. Half-open UTF-8 byte ranges against the expected snapshot, sorted and non-overlapping. Reject out-of-bounds offsets, split scalars and stale revisions before mutating. Ordered same-position inserts are allowed; an empty batch leaves the revision unchanged. Two edits touching the same boundary are applied in listed order. |
| `DocumentRef { workspace, document, revision }` | Planned. Opaque IDs scoped to instance/workspace; revision increases even on undo/redo. Checked arithmetic. View state is keyed by document and view ID. IDs survive rename within an active workspace. |
| `RangeCodec::convert(snapshot, range, encoding)` | Planned. Line semantics are protocol-specific: LF/CRLF/CR for LSP, Tree-sitter points count LF and bytes. Validate UTF-8 and UTF-16 boundaries, handle surrogate pairs and report unmappable sequences. Grapheme movement and tab-expanded display columns stay in layout. |
| `Save(document, revision, expected_disk_stamp)` | Planned. Record saved revision and file identity/hash as needed. On external change, present reload/diff/save-as. Write to a temporary sibling, flush, use OS atomic replacement where supported and sync parent metadata as applicable. Preserve relevant permissions; test ACLs and explicit symlink policy. Handle save races with a last-moment file check and report that generic filesystems offer no universal compare-and-swap. Newer edits remain dirty. |
| `WorkspaceEdit { id, edits_by_document, resource_ops }` | Planned. Stage, validate revision and root containment for every operation, show preview, then apply under the coordinator. Buffer-only changes commit as one batch and undo together. Resource operations need a journal and compensating recovery with explicit partial-failure reporting. Disable those operations until that recovery path passes fault injection. |
| `WorkerEnvelope { version, request_id, workspace_id, generation, payload }` | Planned local framed IPC. LSP/DAP keep their own framing. Bound frame and queue sizes, validate inputs, correlate results, and distinguish cancel, timeout, process exit and protocol failure. Negotiate major/minor version and feature flags; reject incompatible majors. Reconnect with fresh generation and explicit snapshot resync. |
| `CapabilityGrant { publisher, package, workspace, scope }` | Planned. Host owns grants and may revoke them at runtime. Extension sees opaque handles, never unrestricted filesystem paths. Extension writes are proposals and must pass normal edit preconditions. |

Recovery records contain workspace and file identity, base file digest, revision, checksummed delta or snapshot, and format version. Flush within one second of the last edit with a periodic flush during continuous typing. Retain dirty documents until saved or explicitly discarded. Ignore only torn trailing records and surface corruption. Recover into an unsaved buffer and show a diff when disk content diverges. Garbage collect after verified checkpointing. Keep recovery artifacts in private app data. Document retention and let users clear recovered content. On file watch overflow, rescan, reconcile with app-initiated writes, and retain dirty buffers and saved state coherently.

## Security model

Treat workspace files, protocol messages, terminal output, extensions, installer feeds and AI output as untrusted. A newly opened folder is restricted: syntax, editor and file browsing are available, while project tool discovery, tasks, LSP/DAP launches and extension callbacks requesting workspace data require grants. Show exact executable/arguments and working directory when granting a new launch capability. Workspace trust keys to a stable root identity, survives controlled rename only after revalidation and is revocable. Local trusted tools have normal user permissions; supervision limits lifecycle and output, not their filesystem authority.

Check paths at the operation boundary. Handle symlinks, Windows reparse points, UNC paths and case aliases using platform-aware canonical identity and handle-based checks where available. Fail closed on races where a safe bounded operation is impossible. Use no network listeners for core services. Strip sensitive inherited environment values for managed helpers where compatible. Avoid logging buffer content, credentials and raw environment. Keep supply-chain provenance for tools and grammars, audit licenses and transitive dependencies, and use signed release metadata with expiry and rollback protection if an updater is introduced. Publish a private security contact before public beta. Document known boundaries and supported security updates in the release policy.

## Testing and release

- Unit and property tests compare random transaction sequences to a reference string and check undo/redo and Unicode conversions. Fixtures cover grapheme clusters, IME text, CRLF, BOM, mixed line endings and pathological long lines. Fuzz edit validation, protocol decoding, crash replay and extension manifest parsing.
- Fake LSP/DAP peers exercise cancellation, partial frames, malformed messages, out-of-order results, restart and resource limits. Contract tests run real pinned language tools on synthetic Rust and TypeScript projects. Integration tests cover PTY interactive commands and resize, exact staging state, workspace trust, extension quotas and save or recovery under forced termination.
- CI on macOS, Windows and Ubuntu runs format, lint, locked tests and integration checks. Build platform artifacts using native runners. Add dedicated or manual desktop checks on GPU-equipped hosts, both Linux display servers, screen readers, IMEs and fractional scaling. CI runner success by itself does not establish desktop parity.
- Add dependency/license checks, advisories, secret scanning, SBOM and build provenance. Untrusted pull requests receive read-only CI without signing secrets. Protect release tags and deployment environments. Use reviewer-approved release promotion, pinned actions/toolchains and access to short-lived signing credentials where supported.
- Ship signed and notarized macOS app in DMG, signed Windows MSI, and Ubuntu x64 DEB with signed repository metadata or signed checksums for standalone downloads. Audit redistribution obligations for every bundled component. Publish hashes, release notes, SBOM and supported-platform matrix. Verify clean install, upgrade from prior supported version, uninstall and preservation of user settings on every target. Early upgrades can use manual downloads. Roll back to prior signed builds only with compatible persisted state or a documented backed-up migration path. Add signed in-app updates after rollback and key-rotation tests pass. Keep beta and stable channels distinct.

## Milestones and exit gates

Each milestone produces a runnable increment or standalone testable crate. Estimates are sequencing units, not calendar dates. Dependencies are cumulative unless stated.

| Milestone | Work | Exit gate |
|---|---|---|
| M0 — foundation | Cargo workspace, document snapshots, transactional edits and CI definition. | Local locked build, lint and tests pass. Once pushed, same checks pass on all three CI runners. |
| M1 — native shell and durability | Time-box toolkit evaluation, choose shell, implement open/edit/save, undo, encodings, recovery and command routing. | Matrix passes keyboard and IME input, screen-reader caret/selection, LF/CRLF byte round-trip, external-change detection and forced crash after flush. No invalid batch partially applies. |
| M2 — workspace and languages | Explorer, search and indexing, Tree-sitter, Rust Analyzer and TypeScript profiles. | Offline fixture tests show diagnostics, completion, definition and previewed rename on both languages. Restart recovers open docs and old results cannot overwrite current state. |
| M3 — run, debug and Git | PTY, task execution, Git panel, Rust/Node DAP profiles. | On all targets, open fixture, edit, run in terminal, hit breakpoint, inspect variables, step, continue and commit only intended changes. User sees tool failure and can restart without losing edits. |
| M4 — extensions and hardening | Wasm SDK, permission UI, restricted mode, packaging and performance profiling. | Denied filesystem/network/spawn requests have no side effects in sandbox tests. A runaway extension is stopped and editor remains interactive. Every platform passes signed installer lifecycle tests. |
| M5 — beta and stable | Pilot on real local projects. Fix blockers; publish support matrix, package verification and recovery guidance. | Full 1.0 suite passes on a fresh install per OS. Zero known critical security, data-loss or supported-workflow blockers. Optional AI proceeds independently through explicit egress and patch review tests. |

### Measurement targets

Record the OS, toolkit, commit, machine model, CPU, RAM, GPU and driver for every run. Baseline with a 16 GB machine and local SSD per platform, at least 20 cold launches and 1,000 input events after warm-up. Use versioned synthetic 10 MB source files and a 10,000-file workspace. Initial budgets are p95 input-to-frame under 33 ms during indexing for ordinary 100 KB files, cold launch-to-editable under 2 s p95 measured over at least 20 cold runs, first editable view of 10 MB fixture under 2 s p95, idle RSS below 300 MB before language servers start and core idle CPU below 2% of one core averaged over 60 s. Include OS composition in input-to-frame measurements where tooling permits and report any instrumentation gaps. Verify committed saves survive process death on the documented filesystems, journaled edits recover exactly once and recovery loss is limited to unflushed edits within the documented flush window under normal storage conditions. Calibrate budgets after M1 measurements and document any changed gate.

### First implementation queue

1. **B01 — Cross-platform baseline.** Run M0 on hosted runners after repository setup, capture results in the first change review, and keep Cargo.lock checked in. Acceptance is the M0 matrix gate.
2. **B02 — Coordinates and document invariants.** Add byte/line/UTF-16 mapping and property cases for reference string parity, CRLF, surrogate halves and arbitrary Unicode batches. Keep protocol-specific line rules explicit. Depends on the text crate and can run in parallel with B03.
3. **B03 — Toolkit decision.** Build one native editor surface and measure input, IME, accessibility and frame timing on all three platforms. Record the tested revision, renderer and fallback decision in an ADR. Depends on access to target systems.
4. **B04 — Durable document lifecycle.** Add document identities, grouped undo/redo, conflict-aware atomic save and recovery. Run kill/restart and disk fault tests. Depends on B02 for document invariants.
5. **B05 — First desktop slice.** Connect B03 and B04 to open/edit/save/reopen and search. Demonstrate the full lifecycle on all three systems before adding service integrations. Integrate a fake LSP peer before installing any real language server by default.
