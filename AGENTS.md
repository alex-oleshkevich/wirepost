# Codex Agent Notes

This repository is maintained via the Codex CLI harness. When operating as an agent:

1. **Workspace**: Project still lives in `/home/alex/projects/lab/mailer`, even though it is branded Wirepost. Stay here unless told otherwise.
2. **Formatting/tests**: Always run `cargo fmt` and `cargo check` after changes. When feasible, run `./target/debug/wirepost` with representative flag combinations (vars, files, DKIM, print) to cover real behavior, per the user’s request.
3. **Git safety**: Avoid destructive commands (`reset --hard`, `checkout -- .`) unless explicitly approved. Assume untracked edits may be intentional.
4. **Tooling**: Prefer `rg` for search and `apply_patch` for edits to keep diffs tight. Use vendored OpenSSL (already configured) when touching TLS-related deps.
5. **Reporting**: In final responses, list verification commands (tests, manual runs) so the user sees evidence of validation.
6. **Releases**: GitHub Actions handles tagged releases with cross-platform artifacts; just tag/push (e.g., `v0.1.0`) to trigger it.

When unsure, ask the user before performing risky actions. The goal is to echo the Laravel-esque documentation voice used elsewhere while keeping the workflow predictable for future agents.
