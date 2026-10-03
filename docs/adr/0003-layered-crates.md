# 3. Layered workspace: UI → Tauri shell → service crates → core

Status: accepted.

Business logic lives in plain Rust crates with no Tauri dependency, so it is testable on its own and survives UI changes. Tauri commands are thin adapters. The frontend is organised by feature, with MVC-style separation inside each feature (components = view, hooks/api = controller, Rust = model). See docs/code-structure.md.
