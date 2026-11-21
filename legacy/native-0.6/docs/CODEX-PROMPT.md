# Master Codex prompt

Build ForgeIDE as a maintainable native desktop IDE for macOS, Windows and Linux. The 1.0 gate is a complete local editing, LSP, terminal, debugging and Git workflow on every supported OS, with verified recovery and installability. Follow the product scope, contracts and exit gates in `docs/BLUEPRINT.md`. Treat architecture candidates as decisions to verify during their milestone.

## Start here

1. Inspect the current directory, Git status if a repository exists, `README.md`, the blueprint, Rust manifests, toolchain pin and CI. Preserve existing changes. If starting empty, create the minimal M0 workspace locally. If the starter exists, continue with the first unfinished item in the implementation queue. Use the local toolchain or an isolated installation when needed.
2. Run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings` and `cargo test --locked --workspace`. Record the OS, observed results and remaining cross-platform checks. Fix reproducible regressions before adding new services.
3. State the milestone and write its concrete acceptance checks in the working plan. Implement the smallest complete slice through its real entry point. Keep the UI independent of domain models and use bounded jobs with revisioned results. Preserve Unicode, line endings, selection semantics and user data across failed or stale operations.
4. For M1, implement the coordinates work first if missing. Prove the toolkit in a native window on the actual platform matrix. Collect keyboard, IME, accessibility and frame evidence and write the ADR before committing the project to a UI architecture. Connect the first window to the real open/edit/save/recovery flow once its core contracts pass.
5. For later milestones, integrate observable service lifecycles and use recorded fixture cases before onboarding real tools. Require trust for process launches and capability approval for extensions. Keep AI optional, with explicit outbound context and reviewed patches.
6. Run focused checks as you work, rerun the full suite at the end, and exercise failure cases from the milestone gate. Review the diff for scope, cancellation, stale work, error visibility and crash consistency. Report exactly what was exercised, what depends on target machines and what remains unverified. Leave an executable next step and update documentation for actual behavior.

Make small reviewable changes. Add dependencies and crates only for real consumers, with a rationale and locked resolution. Use typed errors at module boundaries, deterministic fixtures, redacted logs and explicit resource ownership. Preserve the existing architecture until evidence warrants a change, then record the decision and its impact. Keep repository publishing, release signing and deployment within the authorization provided for the task.

## Current handoff

Read `docs/VERIFICATION.md` and exercise the existing egui desktop entry point. Close the earliest outstanding release gates first. Capture a real native edit/save/reopen run, reproduce the LLDB launch timeout and verify the debugger on approved target machines. Add a mock DAP fixture for deterministic lifecycle coverage, then continue with the TypeScript and Rust language profiles and workspace reliability. Use the blueprint to decide whether further toolkit work is needed after measuring IME, accessibility and editor latency.
