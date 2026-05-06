//! Phase 10: Performance Hardening
//!
//! - Criterion benchmark suite
//! - Memory profiling (dhat)
//! - Incremental scan (skip unchanged dirs)
//! - Async I/O tuning (async hashing, buffered reads)
//! - Database query optimization (WAL, indexes, batch inserts)

pub mod bench;
pub mod incremental;
pub mod async_io;
pub mod db_opt;

pub use bench::*;
pub use incremental::*;
pub use async_io::*;
pub use db_opt::*;
