//! Stress tests: large-vault search, concurrent atomic writes, and bulk
//! wiki-link rewriting. These use generous time bounds so they stay green on
//! slow CI machines while still catching order-of-magnitude regressions.
//!
//! The module itself is `#[cfg(test)]` at the lib.rs declaration site, so
//! we don't repeat it here.
//! Each test uses an isolated temporary Forge and asserts safety invariants in
//! addition to generous upper bounds; timing thresholds are regression alarms,
//! not performance benchmarks.

/// Upper bound for every wall-clock assertion in the crate's test suites, not
/// just this module's. These are order-of-magnitude alarms, not benchmarks: the
/// operations they guard run in well under a second locally, so a breach means
/// something regressed by a factor of hundreds.
///
/// Sized for the slowest platform we test on rather than the fastest. The
/// Windows CI runner measured 25s for the 500-note ZIP round trip and 31s for
/// the 550-link rewrite on healthy code, roughly ten times the macOS figures,
/// because these suites are dominated by per-file I/O. A budget tuned to a Mac
/// simply reports Windows as broken.
pub(crate) const REGRESSION_BUDGET_SECS: u64 = 120;

/// How many times slower than its own same-run baseline an operation may be.
///
/// A wall-clock ceiling alone cannot tell a real regression from a busy
/// machine: raising it until CI stops flaking is what turned a 5-second budget
/// into two minutes, and two minutes catches almost nothing. So each timing
/// test also measures the irreducible work it depends on — reading the same
/// files, parsing the same links, scanning the same Forge — inside the same
/// run, and asserts a ratio against that. Load slows both halves together, so
/// the ratio holds on a loaded runner while still failing on the
/// order-of-magnitude regressions these tests exist to catch. The measured
/// medians on a Linux desktop, idle or loaded, sit between 1.2 and 2.4 for
/// the search and between 2.0 and 3.3 for the link rewrite, so this leaves six
/// times headroom or more. The index-versus-scan check has its own bar,
/// [`INDEX_SPEEDUP_FLOOR`].
///
/// Both sides of every ratio are medians of interleaved runs, never single
/// samples: see [`interleaved_medians`].
const SAME_RUN_RATIO_LIMIT: f64 = 20.0;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::commands::search::search_notes_content_in;
use crate::persist::write_atomic;
use crate::wiki::rewrite_links_for_rename;

struct TempVault(PathBuf);

impl TempVault {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "moldavite-stress-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        for sub in ["notes", "notes/Projects", "daily", "weekly"] {
            fs::create_dir_all(base.join(sub)).unwrap();
        }
        Self(base)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn lorem_note(i: usize, with_needle: bool) -> String {
    let mut s = format!("# Note {i}\n\n");
    for para in 0..8 {
        s.push_str(&format!(
            "Paragraph {para} of note {i}: the slow amber fox considers \
             [[note-{}]] and drifts along the ridge line without hurry.\n\n",
            (i + para) % 1000
        ));
    }
    if with_needle {
        s.push_str("This one mentions the moldavite-needle exactly once.\n");
    }
    s
}

/// Wall-clock time of one call.
fn timed(op: impl FnOnce()) -> Duration {
    let started = Instant::now();
    op();
    started.elapsed()
}

fn median(mut samples: Vec<Duration>) -> Duration {
    debug_assert!(samples.len() % 2 == 1, "an odd count has a true median");
    samples.sort_unstable();
    samples[samples.len() / 2]
}

/// Median times of `baseline` and `measured`, after one untimed warm-up of
/// `measured`, over `rounds` rounds of one `baseline` run followed by
/// `measured_per_round` runs of `measured`.
///
/// A single sample is at the mercy of the scheduler: on a shared CI runner one
/// stall of a few tens of milliseconds inside an operation that takes a few
/// milliseconds reads as a tenfold slowdown, the likely cause of the
/// index-versus-scan failure on a macOS runner (issue #26). A median needs most samples to be slow before it
/// moves. Interleaving puts both halves of a ratio under the same load, so a
/// burst from a parallel test lands on both sides rather than on whichever one
/// happened to be measuring.
fn interleaved_medians(
    rounds: usize,
    measured_per_round: usize,
    mut baseline: impl FnMut(),
    mut measured: impl FnMut(),
) -> (Duration, Duration) {
    measured();
    let mut baseline_times = Vec::with_capacity(rounds);
    let mut measured_times = Vec::with_capacity(rounds * measured_per_round);
    for _ in 0..rounds {
        baseline_times.push(timed(&mut baseline));
        for _ in 0..measured_per_round {
            measured_times.push(timed(&mut measured));
        }
    }
    (median(baseline_times), median(measured_times))
}

#[test]
fn stress_search_over_1000_note_vault() {
    let vault = TempVault::new("search");
    let base = vault.path();

    for i in 0..1000 {
        let dir = if i % 5 == 0 {
            "notes/Projects"
        } else {
            "notes"
        };
        let content = lorem_note(i, i % 100 == 0); // 10 notes carry the needle
        fs::write(base.join(dir).join(format!("note-{i}.md")), content).unwrap();
    }

    // Same-run floor: every note read once, which is the irreducible work the
    // search has to do. Both run against the same warm page cache.
    let (read_all, elapsed) = interleaved_medians(
        5,
        1,
        || {
            let mut baseline_bytes = 0usize;
            for i in 0..1000 {
                let dir = if i % 5 == 0 {
                    "notes/Projects"
                } else {
                    "notes"
                };
                baseline_bytes += fs::read_to_string(base.join(dir).join(format!("note-{i}.md")))
                    .unwrap()
                    .len();
            }
            assert!(baseline_bytes > 0);
        },
        || {
            let results =
                search_notes_content_in(base, &base.join(".trash"), "moldavite-needle", 50);
            assert_eq!(results.len(), 10, "expected exactly the 10 seeded matches");
        },
    );

    let ratio = elapsed.as_secs_f64() / read_all.as_secs_f64();
    eprintln!(
        "[stress] search over 1000 notes took {elapsed:?}; reading them all took {read_all:?} \
         (medians, ratio {ratio:.2})"
    );
    assert!(
        ratio < SAME_RUN_RATIO_LIMIT,
        "search over 1000 notes took {elapsed:?}, {ratio:.1}x the {read_all:?} it takes to read \
         them — order-of-magnitude regression"
    );
    assert!(
        elapsed.as_secs() < REGRESSION_BUDGET_SECS,
        "search over 1000 notes took {elapsed:?} — order-of-magnitude regression"
    );
}

/// Scan versus persistent index on a Forge far past the size the live scan
/// was designed for, plus the cost of the reconcile that builds it.
///
/// The query bound is relative to the scan measured in the same run, so a
/// loaded CI runner — an order of magnitude slower than a warm laptop — slows
/// both sides and the assertion still means what it says.
fn stress_index_versus_scan(note_count: usize) {
    let vault = TempVault::new(&format!("index-{note_count}"));
    let base = vault.path();

    for i in 0..note_count {
        let dir = if i % 5 == 0 {
            "notes/Projects"
        } else {
            "notes"
        };
        let content = lorem_note(i, i % 100 == 0);
        fs::write(base.join(dir).join(format!("note-{i}.md")), content).unwrap();
    }
    let expected = note_count.div_ceil(100);

    // The cold build is the one asserted measurement taken once: repeating it
    // means dropping and rebuilding the whole index, most of this test's
    // runtime. It keeps a fixed ceiling instead of a ratio. Measured: 2.6 s on
    // a Linux desktop, up to 13 s pinned to three loaded CPUs, 3.5 s on a
    // macOS runner; 60 s still catches a regression of about twentyfold.
    let started = Instant::now();
    let indexed_count = crate::search_index::reconcile(base).unwrap();
    let reconcile_time = started.elapsed();
    assert_eq!(indexed_count as usize, note_count);

    // A no-op reconcile is the common case: nothing changed, so every note
    // should be settled by its `(mtime, size)` alone.
    let started = Instant::now();
    crate::search_index::reconcile(base).unwrap();
    let warm_reconcile = started.elapsed();

    // Three scans, each followed by three index queries: a scan costs as much
    // as twenty-odd queries, so the index gets the extra samples cheaply.
    let (scan_query, index_query) = interleaved_medians(
        3,
        3,
        || {
            let scanned = crate::commands::search::scan_notes_content_in(
                base,
                &base.join(".trash"),
                "moldavite-needle",
                500,
            );
            assert_eq!(scanned.len(), expected);
        },
        || {
            let hits = crate::search_index::query(base, base, "moldavite-needle", 500).unwrap();
            assert_eq!(hits.len(), expected);
        },
    );

    let speedup = scan_query.as_secs_f64() / index_query.as_secs_f64();
    eprintln!(
        "[stress] {note_count} notes — scan query {scan_query:?}, index query {index_query:?} \
         (medians, {speedup:.1}x), cold reconcile {reconcile_time:?}, warm reconcile \
         {warm_reconcile:?}"
    );
    crate::search_index::delete_for(base);

    // Against the scan measured in the same run, not a fixed millisecond
    // ceiling a loaded runner can blow through on healthy code. The index is
    // there to be an order of magnitude faster than walking the Forge; that is
    // the claim, and it is the one worth failing on.
    assert!(
        speedup > INDEX_SPEEDUP_FLOOR,
        "index query over {note_count} notes took {index_query:?} against a {scan_query:?} scan \
         (medians, {speedup:.1}x) — the index is no longer an order of magnitude faster"
    );
    assert!(
        reconcile_time.as_secs() < 60,
        "reconcile of {note_count} notes took {reconcile_time:?}"
    );
}

/// How many times faster than the scan the index query has to be, comparing
/// the medians from [`interleaved_medians`].
///
/// Measured over 10,000 notes in the debug build `cargo test` runs, scan
/// median divided by index median (2026-10):
///
/// - Linux, 6-core desktop: 27-34x over 10 runs.
/// - The same with twelve busy loops on its six cores: 21-34x over 30.
/// - Pinned to 3 CPUs, a macOS runner's count, beside a looping full `cargo
///   test --lib` and two busy loops: 19-40x over 60. The scan there takes
///   120-460 ms, around the runner's 359 ms.
/// - The same pinned to 2 CPUs: 24-30x over 30.
/// - macOS runner: medians not yet logged (CI hides a passing test's output;
///   issue #26 tracks collecting them). The one figure is the single sample
///   this replaced: 6.7x (scan 359 ms, index 54 ms). On the 3-CPU set-up single samples fell as low as
///   8.7x, failing 1 run in 60, while the medians never went below 19x.
///
/// The lowest median, 19x, still clears the bar about twice over, so CI keeps
/// the same bar as a laptop. A query that lost its index scores about 1x.
const INDEX_SPEEDUP_FLOOR: f64 = 10.0;

#[test]
fn stress_index_versus_scan_over_10000_note_vault() {
    stress_index_versus_scan(10_000);
}

/// The 50,000-note figure is a deliberate outlier: it writes about 45 MB
/// across 50,000 files and takes the better part of a minute, so it runs only
/// when asked for. Measured on an Apple Silicon SSD on 2026-09-04: scan query
/// 1.33 s, index query 11 ms, cold reconcile 8.4 s, warm reconcile 477 ms.
#[test]
fn stress_index_versus_scan_over_50000_note_vault() {
    if std::env::var("LARIMAR_STRESS_LARGE").as_deref() != Ok("1") {
        eprintln!("[stress] 50,000-note run skipped; set LARIMAR_STRESS_LARGE=1 to include it");
        return;
    }
    stress_index_versus_scan(50_000);
}

#[test]
fn stress_atomic_writes_concurrent_distinct_files() {
    let vault = TempVault::new("atomic-distinct");
    let base = vault.path().join("notes");

    let started = Instant::now();
    std::thread::scope(|scope| {
        for t in 0..8 {
            let base = base.clone();
            scope.spawn(move || {
                for i in 0..100 {
                    let path = base.join(format!("t{t}-n{i}.md"));
                    let body = format!("thread {t} note {i}\n{}", "x".repeat(512));
                    write_atomic(&path, body.as_bytes(), Some(0o600)).unwrap();
                }
            });
        }
    });
    eprintln!(
        "[stress] 800 concurrent atomic writes took {:?}",
        started.elapsed()
    );

    let mut count = 0;
    for entry in fs::read_dir(&base).unwrap().flatten() {
        if !entry.path().is_file() {
            continue;
        }
        let content = fs::read_to_string(entry.path()).unwrap();
        assert!(
            content.starts_with("thread "),
            "torn or empty file: {:?}",
            entry.path()
        );
        count += 1;
    }
    assert_eq!(count, 800);
}

#[test]
fn stress_atomic_writes_same_file_never_torn() {
    let vault = TempVault::new("atomic-same");
    let path = vault.path().join("notes").join("contended.md");

    // Two full payloads of different lengths; a torn write would produce a
    // file matching neither.
    let a = format!("AAAA {}\n", "a".repeat(4096));
    let b = format!("BB {}\n", "b".repeat(9000));

    std::thread::scope(|scope| {
        let (path_a, payload_a) = (path.clone(), a.clone());
        scope.spawn(move || {
            for _ in 0..150 {
                write_atomic(&path_a, payload_a.as_bytes(), None).unwrap();
            }
        });
        let (path_b, payload_b) = (path.clone(), b.clone());
        scope.spawn(move || {
            for _ in 0..150 {
                write_atomic(&path_b, payload_b.as_bytes(), None).unwrap();
            }
        });
    });

    let final_content = fs::read_to_string(&path).unwrap();
    assert!(
        final_content == a || final_content == b,
        "file is torn: {} bytes, expected {} or {}",
        final_content.len(),
        a.len(),
        b.len()
    );

    // No stray temp files left behind.
    let leftovers: Vec<_> = fs::read_dir(vault.path().join("notes"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
}

#[test]
fn stress_rewrite_links_across_large_corpus() {
    // 500 notes, each with 9 links; rewrite one target across all of them.
    let corpus: Vec<String> = (0..500).map(|i| lorem_note(i, false)).collect();

    let mut touched = 0;
    let (parse_all, elapsed) = interleaved_medians(
        5,
        1,
        || {
            let mut found = 0usize;
            for content in &corpus {
                found += crate::wiki::parse_wiki_links(content).len();
            }
            assert!(found > 0);
        },
        || {
            touched = 0;
            for content in &corpus {
                if let Some(rewritten) =
                    rewrite_links_for_rename(content, "note-42", "renamed-note", false)
                {
                    assert!(rewritten.contains("[[renamed-note]]"));
                    assert!(!rewritten.contains("[[note-42]]"));
                    touched += 1;
                }
            }
        },
    );
    let ratio = elapsed.as_secs_f64() / parse_all.as_secs_f64();
    eprintln!(
        "[stress] link rewrite across 500 notes took {elapsed:?} ({touched} touched); parsing \
         the same links took {parse_all:?} (medians, ratio {ratio:.2})"
    );
    // note i links to (i+para)%1000 for para 0..8 — several notes link to 42.
    assert!(touched > 0, "expected at least one note to link to note-42");
    assert!(
        ratio < SAME_RUN_RATIO_LIMIT,
        "link rewrite across 500 notes took {elapsed:?}, {ratio:.1}x the {parse_all:?} it takes \
         to parse the same links — order-of-magnitude regression"
    );
    assert!(
        elapsed.as_secs() < REGRESSION_BUDGET_SECS,
        "link rewrite across 500 notes took {elapsed:?} — order-of-magnitude regression"
    );
}
