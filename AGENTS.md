# Codex Agent Notes

This repository is maintained via the Codex CLI harness. When operating as an agent:

1. **Stay in `/home/alex/projects/lab/mailer`** – repository is named Wirepost but currently lives at this path; keep edits here unless instructed otherwise.
2. **Use `cargo fmt` and `cargo check`** after changes. These keep the Rust codebase clean and catch compiler errors early.
3. **Avoid destructive git commands** (`reset --hard`, `checkout -- .`) unless the user asks for them. The working tree may contain intentional local edits.
4. **Prefer `rg` for searching** and `apply_patch` for local file edits. The CLI expects concise, high-signal edits rather than large file dumps.
5. **Describe verification steps** in your final response (tests run, commands executed) so the user can see what was validated.

When unsure, ask the user before performing risky actions. The goal is to echo the Laravel-esque documentation voice used elsewhere while keeping the workflow predictable for future agents.
