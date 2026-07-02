//! Sampling-profiler harness: runs one named workload in a loop under pprof
//! and emits a flamegraph SVG plus self/inclusive hotspot tables.
//!
//! Usage:
//!   cargo run --profile profiling -p dmp-bench --bin profile -- --list
//!   cargo run --profile profiling -p dmp-bench --bin profile -- <scenario> [seconds]
//!
//! Workloads mirror benches/dmp.rs (same generators), so a flamegraph here
//! explains the corresponding criterion number. Scenarios whose workload
//! mutates its input clone it inside the loop; the Clone frames are visible
//! in the flamegraph and marked with * in --list.

use diff_match_patch::Dmp;
use dmp_bench as ds;
use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use std::time::{Duration, Instant};

const NAMES: &[&str] = &[
    "diff_scattered",
    "diff_html",
    "diff_code_rename",
    "diff_many_small",
    "diff_unique_lines",
    "diff_cjk",
    "diff_soup",
    "diff_disjoint",
    "diff_random_binary",
    "diff_repetitive_deadline",
    "cleanup_semantic*",
    "cleanup_lossless*",
    "cleanup_merge*",
    "patch_make",
    "patch_apply_clean",
    "patch_apply_shifted",
    "patch_apply_fuzzed",
    "wire_roundtrip",
    "match_bitap",
];

fn diff_workload(pair: (String, String), timeout: Option<f32>) -> Box<dyn FnMut()> {
    Box::new(move || {
        let mut d = Dmp::new();
        d.diff_timeout = timeout;
        black_box(d.diff_main(&pair.0, &pair.1, true));
    })
}

fn build(name: &str) -> Option<Box<dyn FnMut()>> {
    Some(match name {
        "diff_scattered" => diff_workload(ds::pair_scattered(), None),
        "diff_html" => diff_workload(ds::pair_html_churn(), None),
        "diff_code_rename" => diff_workload(ds::pair_code_rename(), None),
        "diff_many_small" => diff_workload(ds::pair_many_small_edits(), None),
        "diff_unique_lines" => diff_workload(ds::pair_unique_lines(), None),
        "diff_cjk" => diff_workload(ds::pair_cjk(), None),
        "diff_soup" => diff_workload(ds::pair_one_line_soup(), None),
        "diff_disjoint" => diff_workload(ds::pair_disjoint(2_000), None),
        "diff_random_binary" => diff_workload(ds::pair_random_binary(4_000), None),
        "diff_repetitive_deadline" => diff_workload(ds::pair_repetitive(), Some(1.0)),
        "cleanup_semantic" => {
            let (t1, t2) = ds::pair_scattered();
            let diffs = Dmp::new().diff_main(&t1, &t2, true);
            Box::new(move || {
                let mut d = Dmp::new();
                let mut work = diffs.clone();
                d.diff_cleanup_semantic(&mut work);
                black_box(work);
            })
        }
        "cleanup_lossless" => Box::new(move || {
            let mut d = Dmp::new();
            let mut work = ds::diffs_lossless_slide(2_000);
            d.diff_cleanup_semantic_lossless(&mut work);
            black_box(work);
        }),
        "cleanup_merge" => Box::new(move || {
            let mut d = Dmp::new();
            let mut work = ds::diffs_merge_churn(1_500);
            d.diff_cleanup_merge(&mut work);
            black_box(work);
        }),
        "patch_make" => {
            let (t1, t2) = ds::pair_scattered();
            Box::new(move || {
                let mut d = Dmp::new();
                black_box(d.patch_make1(&t1, &t2));
            })
        }
        "patch_apply_clean" | "patch_apply_shifted" | "patch_apply_fuzzed" => {
            let (t1, t2) = ds::pair_scattered();
            let mut patches = Dmp::new().patch_make1(&t1, &t2);
            let source = match name {
                "patch_apply_shifted" => ds::shifted_source(&t1),
                "patch_apply_fuzzed" => ds::fuzz_patch_contexts(&t1, &patches),
                _ => t1,
            };
            Box::new(move || {
                let mut d = Dmp::new();
                black_box(d.patch_apply(&mut patches, &source));
            })
        }
        "wire_roundtrip" => {
            let (t1, t2) = ds::pair_scattered();
            let mut d = Dmp::new();
            let mut diffs = d.diff_main(&t1, &t2, true);
            let mut patches = d.patch_make1(&t1, &t2);
            Box::new(move || {
                let mut d = Dmp::new();
                let delta = d.diff_todelta(&mut diffs);
                black_box(d.diff_from_delta(&t1, &delta));
                let text = d.patch_to_text(&mut patches);
                black_box(d.patch_from_text(text));
            })
        }
        "match_bitap" => {
            let (hay, pattern, loc) = ds::match_case();
            Box::new(move || {
                let mut d = Dmp::new();
                black_box(d.match_main(&hay, &pattern, loc));
            })
        }
        _ => return None,
    })
}

/// Strip the rustc symbol hash (`...::h0123456789abcdef`).
fn clean_name(sym: &str) -> String {
    match sym.rfind("::h") {
        Some(i) if sym.len() - i == 19 && sym[i + 3..].bytes().all(|b| b.is_ascii_hexdigit()) => {
            sym[..i].to_string()
        }
        _ => sym.to_string(),
    }
}

fn print_top(title: &str, counts: &HashMap<String, isize>, total: isize, top: usize) {
    let mut rows: Vec<(&String, &isize)> = counts.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    println!("\n{title} (of {total} samples)");
    for (name, n) in rows.into_iter().take(top) {
        println!("  {:>6.2}%  {}", 100.0 * *n as f64 / total as f64, name);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = args.first().map(String::as_str).unwrap_or("--list");
    if name == "--list" || name == "-l" {
        println!("scenarios (* = clones its input inside the loop):");
        for n in NAMES {
            println!("  {n}");
        }
        return;
    }
    let seconds: f64 = args
        .get(1)
        .map(|s| s.parse().expect("seconds"))
        .unwrap_or(8.0);
    let mut work = build(name.trim_end_matches('*')).unwrap_or_else(|| {
        eprintln!("unknown scenario '{name}'; try --list");
        std::process::exit(2);
    });

    // Touch everything once before sampling starts.
    let warm = Instant::now() + Duration::from_millis(300);
    while Instant::now() < warm {
        work();
    }

    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(997)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .expect("start profiler");
    let started = Instant::now();
    let deadline = started + Duration::from_secs_f64(seconds);
    let mut iters: u64 = 0;
    while Instant::now() < deadline {
        work();
        iters += 1;
    }
    let elapsed = started.elapsed();
    let report = guard.report().build().expect("build profile report");

    std::fs::create_dir_all("target/profiles").expect("create target/profiles");
    let svg = format!("target/profiles/{name}.svg");
    report
        .flamegraph(std::fs::File::create(&svg).expect("create svg"))
        .expect("write flamegraph");

    let mut total: isize = 0;
    let mut self_counts: HashMap<String, isize> = HashMap::new();
    let mut incl_counts: HashMap<String, isize> = HashMap::new();
    for (frames, n) in report.data.iter() {
        total += *n;
        let stack: Vec<String> = frames
            .frames
            .iter()
            .flat_map(|frame| frame.iter().map(|sym| clean_name(&sym.name())))
            .collect();
        if let Some(leaf) = stack.first() {
            *self_counts.entry(leaf.clone()).or_insert(0) += *n;
        }
        let mut seen: HashSet<&String> = HashSet::new();
        for sym in &stack {
            if seen.insert(sym) {
                *incl_counts.entry(sym.clone()).or_insert(0) += *n;
            }
        }
    }

    println!(
        "{name}: {iters} iterations in {:.2}s ({:.3} ms/iter)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e3 / iters as f64,
    );
    println!("flamegraph: {svg}");
    if total == 0 {
        println!("no samples collected — workload too short?");
        return;
    }
    print_top("self time", &self_counts, total, 25);
    print_top("inclusive time", &incl_counts, total, 25);
}
