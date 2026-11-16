//! `peram`: compat alias for `ensembly` (see RENAME.md). Same source, its own bin file,
//! so Cargo does not warn that `src/main.rs` sits in two bin targets.

#[path = "../main.rs"]
mod real;

fn main() -> anyhow::Result<()> {
    real::main()
}
