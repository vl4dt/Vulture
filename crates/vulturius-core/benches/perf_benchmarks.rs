//! Criterion benchmark harness for Vulturius performance testing.
//!
//! Run with: `cargo bench --bench perf_benchmarks`

use criterion::{black_box, BenchmarkId, Criterion};
use std::io::Write;
use std::path::PathBuf;

use vulturius_core::scanner::FileClassifier;
use vulturius_core::types::FileCategory;
use vulturius_core::perf::bench::{classifier_test_paths, make_file_entries};
use vulturius_core::perf::async_io::{AsyncHasher, AsyncIoConfig};
use vulturius_core::perf::incremental::{DirectorySnapshot, IncrementalScanState};
use vulturius_core::perf::db_opt::{BatchInserter, DbOptimizer};

// ─── Classifier Benchmarks ───────────────────────────────────────────────────

fn bench_classifier(c: &mut Criterion) {
    let mut group = c.benchmark_group("classifier");
    let classifier = FileClassifier::new();
    let paths = classifier_test_paths();

    group.bench_function("classify_20_paths", |b| {
        b.iter(|| {
            for path in &paths {
                black_box(classifier.classify(path));
            }
        })
    });

    for &count in &[100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("classify_batch", count), &count, |b, n: &usize| {
            let repeated = vec![paths.clone(); (*n + paths.len() - 1) / paths.len()];
            b.iter(|| {
                let mut total = 0usize;
                for batch in &repeated {
                    for path in batch {
                        total += classifier.classify(path) as usize;
                    }
                }
                black_box(total);
            })
        });
    }

    group.finish();
}

// ─── File Entry Benchmarks ───────────────────────────────────────────────────

fn bench_file_entry_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_entry");

    for &count in &[100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("create_entries", count), &count, |b, n: &usize| {
            b.iter(|| {
                let entries = make_file_entries(*n);
                black_box(entries);
            })
        });
    }

    group.finish();
}

// ─── Safety Score Benchmarks ─────────────────────────────────────────────────

fn bench_safety_score(c: &mut Criterion) {
    let mut group = c.benchmark_group("safety_score");

    let categories = [
        FileCategory::Temp,
        FileCategory::Cache,
        FileCategory::Log,
        FileCategory::Document,
        FileCategory::Image,
        FileCategory::Executable,
        FileCategory::Unknown,
    ];

    group.bench_function("safety_score_all_categories", |b| {
        b.iter(|| {
            for cat in &categories {
                black_box(cat.default_safety_score());
            }
        })
    });

    group.finish();
}

// ─── Hashing Benchmarks ──────────────────────────────────────────────────────

fn bench_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group!("hashing");

    let mut small_file = tempfile::NamedTempFile::new().unwrap();
    write!(small_file, "small file content for hashing benchmark").unwrap();

    let mut medium_file = tempfile::NamedTempFile::new().unwrap();
    write!(medium_file, "{:0>1024}", "medium").unwrap();

    let mut large_file = tempfile::NamedTempFile::new().unwrap();
    write!(large_file, "{:0>65536}", "large").unwrap();

    let small_path = small_file.path().to_path_buf();
    let medium_path = medium_file.path().to_path_buf();
    let large_path = large_file.path().to_path_buf();

    group.bench_function("hash_small_4KB_buf", |b| {
        b.iter(|| {
            black_box(AsyncHasher::hash_file_sync(&small_path, 4096).unwrap());
        })
    });

    group.bench_function("hash_medium_4KB_buf", |b| {
        b.iter(|| {
            black_box(AsyncHasher::hash_file_sync(&medium_path, 4096).unwrap());
        })
    });

    group.bench_function("hash_large_64KB_buf", |b| {
        b.iter(|| {
            black_box(AsyncHasher::hash_file_sync(&large_path, 65536).unwrap());
        })
    });

    for &buf_size in &[1024, 4096, 16384, 65536] {
        group.bench_with_input(
            BenchmarkId::new("hash_buffer_size", buf_size),
            &buf_size,
            |b, size: &usize| {
                b.iter(|| {
                    black_box(AsyncHasher::hash_file_sync(&medium_path, *size).unwrap());
                })
            },
        );
    }

    group.finish();
}

// ─── Incremental Scan Benchmarks ─────────────────────────────────────────────

fn bench_incremental_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group!("incremental_scan");

    let temp_dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..100)
        .map(|i| temp_dir.path().join(format!("dir_{i}")))
        .collect();

    let mut snapshot = DirectorySnapshot::new();
    for path in &paths {
        if let Ok(meta) = std::fs::metadata(path) {
            snapshot.record_dir(path, &meta);
        }
    }
    if let Ok(meta) = std::fs::metadata(temp_dir.path()) {
        snapshot.record_dir(temp_dir.path(), &meta);
    }

    group.bench_function("snapshot_record_dir", |b| {
        b.iter(|| {
            for path in &paths {
                if let Ok(meta) = std::fs::metadata(path) {
                    snapshot.record_dir(path, &meta);
                }
            }
        })
    });

    group.bench_function("snapshot_check_unchanged", |b| {
        b.iter(|| {
            for path in &paths {
                if let Ok(meta) = std::fs::metadata(path) {
                    black_box(snapshot.is_dir_unchanged(path, &meta));
                }
            }
        })
    });

    group.bench_function("incremental_state_check", |b| {
        let prev = DirectorySnapshot::new();
        b.iter(|| {
            let mut state = IncrementalScanState::new(true, Some(prev.clone()));
            for path in &paths {
                if let Ok(meta) = std::fs::metadata(path) {
                    black_box(state.check_dir(path, &meta));
                }
            }
        })
    });

    group.finish();
}

// ─── Batch Inserter Benchmarks ───────────────────────────────────────────────

fn bench_batch_inserter(c: &mut Criterion) {
    let mut group = c.benchmark_group!("batch_inserter");

    for &batch_size in &[100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("insert_batch", batch_size),
            &batch_size,
            |b, size: &usize| {
                b.iter(|| {
                    let mut inserter = BatchInserter::new(*size);
                    for i in 0..10_000 {
                        inserter.push(i);
                        if inserter.is_full() {
                            inserter.take_batch();
                        }
                    }
                    if !inserter.is_empty() {
                        inserter.take_batch();
                    }
                    black_box(inserter.total());
                })
            },
        );
    }

    group.finish();
}

// ─── Memory Allocation Benchmarks ────────────────────────────────────────────

fn bench_memory_profile(c: &mut Criterion) {
    let mut group = c.benchmark_group!("memory");

    group.bench_function("file_entry_allocation_1k", |b| {
        b.iter(|| {
            let entries = make_file_entries(1_000);
            black_box(entries);
        })
    });

    group.bench_function("file_entry_allocation_10k", |b| {
        b.iter(|| {
            let entries = make_file_entries(10_000);
            black_box(entries);
        })
    });

    group.bench_function("classifier_path_allocation", |b| {
        b.iter(|| {
            let paths = classifier_test_paths();
            let classifier = FileClassifier::new();
            let results: Vec<_> = paths.iter().map(|p| classifier.classify(p)).collect();
            black_box(results);
        })
    });

    group.finish();
}

// ─── DB Optimizer Benchmarks ─────────────────────────────────────────────────

fn bench_db_optimizer(c: &mut Criterion) {
    let mut group = c.benchmark_group!("db_optimizer");

    group.bench_function("index_creation_overhead", |b| {
        b.iter(|| {
            black_box(DbOptimizer::create_performance_indexes);
        })
    });

    group.finish();
}

criterion::criterion_group!(
    benches,
    bench_classifier,
    bench_file_entry_creation,
    bench_safety_score,
    bench_hashing,
    bench_incremental_scan,
    bench_batch_inserter,
    bench_memory_profile,
    bench_db_optimizer,
);

criterion::criterion_main!(benches);
