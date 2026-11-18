//! Report machine label must never leak the real hostname.

use std::sync::{Mutex, MutexGuard};

static ENV: Mutex<()> = Mutex::new(());

fn lock_env() -> MutexGuard<'static, ()> {
    ENV.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn defaults_to_unspecified_without_env() {
    let _g = lock_env();
    std::env::remove_var("RETRIEVAL_BENCH_MACHINE");
    assert_eq!(retrieval_bench::machine_label(), "unspecified");
}

#[test]
fn uses_explicit_alias_from_env() {
    let _g = lock_env();
    std::env::set_var("RETRIEVAL_BENCH_MACHINE", "laptop-1");
    assert_eq!(retrieval_bench::machine_label(), "laptop-1");
    std::env::remove_var("RETRIEVAL_BENCH_MACHINE");
}
