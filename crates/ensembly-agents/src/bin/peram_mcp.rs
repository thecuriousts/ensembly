//! `peram-mcp`: compat alias for `ensembly-mcp` (see RENAME.md). Same source, its own bin
//! file, so Cargo does not warn that one file sits in two bin targets.

#[path = "ensembly_mcp.rs"]
mod real;

fn main() {
    real::main()
}
