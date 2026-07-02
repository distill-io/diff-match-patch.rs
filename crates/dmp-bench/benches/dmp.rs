//! Criterion suite for diff_match_patch.
//!
//! Groups:
//!   diff              realistic change shapes at web-monitoring sizes
//!   diff_heavy        realistic but adversarial (dense edits, unique lines)
//!   diff_pathological worst cases (disjoint alphabets, random binary,
//!                     repetitive half-match traps), each also with a
//!                     deadline where the deadline changes the code path
//!   cleanup           cleanup passes on constructed inputs (input cloning
//!                     is excluded via iter_batched)
//!   patch             patch_make + patch_apply on exact/shifted/fuzzed text
//!   wire              delta and patch-text encode/decode
//!   match             match_main / bitap
//!
//! All datasets come from dmp-bench's deterministic generators, shared with
//! the pprof harness (src/bin/profile.rs) so profiles match these workloads.

use criterion::measurement::WallTime;
use criterion::{
    criterion_group, criterion_main, BatchSize, BenchmarkGroup, Criterion, Throughput,
};
use diff_match_patch::Dmp;
use dmp_bench as ds;
use std::hint::black_box;
use std::time::Duration;

fn bench_diff(
    g: &mut BenchmarkGroup<'_, WallTime>,
    id: &str,
    pair: &(String, String),
    timeout: Option<f32>,
) {
    let (t1, t2) = pair;
    g.throughput(Throughput::Bytes((t1.len() + t2.len()) as u64));
    g.bench_function(id, |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.diff_timeout = timeout;
            d.diff_main(black_box(t1), black_box(t2), true)
        })
    });
}

/// The pre-restructure bench ids, kept verbatim so historical baselines
/// remain comparable.
fn legacy(c: &mut Criterion) {
    let pair = ds::pair_interleaved(12);
    let (t1, t2) = &pair;
    c.bench_function("diff_main/interleaved-2k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.diff_main(black_box(t1), black_box(t2), true)
        })
    });
    c.bench_function("patch_make+apply/interleaved-2k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            let mut patches = d.patch_make1(t1, t2);
            d.patch_apply(&mut patches, t1)
        })
    });
}

fn diff_realistic(c: &mut Criterion) {
    let mut g = c.benchmark_group("diff");
    g.warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(30);
    bench_diff(&mut g, "small_edit_50k", &ds::pair_small_edit(), None);
    bench_diff(&mut g, "scattered_edits_50k", &ds::pair_scattered(), None);
    bench_diff(&mut g, "html_churn_90k", &ds::pair_html_churn(), None);
    bench_diff(&mut g, "append_50k", &ds::pair_append(), None);
    bench_diff(&mut g, "prepend_50k", &ds::pair_prepend(), None);
    bench_diff(&mut g, "block_move_60k", &ds::pair_block_move(), None);
    bench_diff(&mut g, "block_delete_60k", &ds::pair_block_delete(), None);
    bench_diff(&mut g, "cjk_scattered_60k", &ds::pair_cjk(), None);
    bench_diff(&mut g, "identical_100k", &ds::pair_identical(), None);
    g.finish();
}

fn diff_heavy(c: &mut Criterion) {
    let mut g = c.benchmark_group("diff_heavy");
    g.warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(3))
        .sample_size(10);
    bench_diff(&mut g, "code_rename_18k", &ds::pair_code_rename(), None);
    // The opt-in word mode on the rename shape (the case it exists for).
    {
        let (t1, t2) = ds::pair_code_rename();
        let mut d = Dmp::new();
        d.word_mode = true;
        let mut diffs = d.diff_main(&t1, &t2, true);
        assert_eq!(
            d.diff_text1(&mut diffs),
            t1,
            "word-mode diff must rebuild text1"
        );
        assert_eq!(
            d.diff_text2(&mut diffs),
            t2,
            "word-mode diff must rebuild text2"
        );
        g.throughput(Throughput::Bytes((t1.len() + t2.len()) as u64));
        g.bench_function("code_rename_18k_words", |b| {
            b.iter(|| {
                let mut d = Dmp::new();
                d.word_mode = true;
                d.diff_main(black_box(&t1), black_box(&t2), true)
            })
        });
    }
    bench_diff(
        &mut g,
        "many_small_edits_22k",
        &ds::pair_many_small_edits(),
        None,
    );
    bench_diff(&mut g, "unique_lines_180k", &ds::pair_unique_lines(), None);
    bench_diff(&mut g, "one_line_soup_16k", &ds::pair_one_line_soup(), None);
    g.finish();
}

fn diff_pathological(c: &mut Criterion) {
    let mut g = c.benchmark_group("diff_pathological");
    g.warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(3))
        .sample_size(10);
    bench_diff(&mut g, "disjoint_2k", &ds::pair_disjoint(2_000), None);
    bench_diff(
        &mut g,
        "random_binary_4k",
        &ds::pair_random_binary(4_000),
        None,
    );
    // A deadline flips on the half-match speedup (and lets bisect give up);
    // same inputs, different code path.
    bench_diff(
        &mut g,
        "random_binary_4k_deadline",
        &ds::pair_random_binary(4_000),
        Some(1.0),
    );
    bench_diff(&mut g, "repetitive_16k", &ds::pair_repetitive(), None);
    bench_diff(
        &mut g,
        "repetitive_16k_deadline",
        &ds::pair_repetitive(),
        Some(1.0),
    );
    g.finish();
}

fn cleanup(c: &mut Criterion) {
    let mut g = c.benchmark_group("cleanup");
    g.warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(20);

    let (t1, t2) = ds::pair_scattered();
    let scattered = Dmp::new().diff_main(&t1, &t2, true);

    g.bench_function("semantic_scattered", |b| {
        b.iter_batched(
            || scattered.clone(),
            |mut diffs| {
                let mut d = Dmp::new();
                d.diff_cleanup_semantic(&mut diffs);
                diffs
            },
            BatchSize::SmallInput,
        )
    });
    g.bench_function("efficiency_scattered", |b| {
        b.iter_batched(
            || scattered.clone(),
            |mut diffs| {
                let mut d = Dmp::new();
                d.edit_cost = 4;
                d.diff_cleanup_efficiency(&mut diffs);
                diffs
            },
            BatchSize::SmallInput,
        )
    });
    g.bench_function("lossless_slide_2k", |b| {
        b.iter_batched(
            || ds::diffs_lossless_slide(2_000),
            |mut diffs| {
                let mut d = Dmp::new();
                d.diff_cleanup_semantic_lossless(&mut diffs);
                diffs
            },
            BatchSize::SmallInput,
        )
    });
    g.bench_function("merge_churn_1500", |b| {
        b.iter_batched(
            || ds::diffs_merge_churn(1_500),
            |mut diffs| {
                let mut d = Dmp::new();
                d.diff_cleanup_merge(&mut diffs);
                diffs
            },
            BatchSize::SmallInput,
        )
    });
    g.finish();
}

fn patch(c: &mut Criterion) {
    let mut g = c.benchmark_group("patch");
    g.warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(20);

    let (t1, t2) = ds::pair_scattered();
    let (h1, h2) = ds::pair_html_churn();

    g.bench_function("make_scattered_50k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.patch_make1(black_box(&t1), black_box(&t2))
        })
    });
    g.bench_function("make_html_90k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.patch_make1(black_box(&h1), black_box(&h2))
        })
    });

    let mut dmp = Dmp::new();
    let mut patches = dmp.patch_make1(&t1, &t2);

    // Sanity outside the timed loops: the three apply scenarios must all
    // take their intended path (clean apply reproduces t2; shifted and
    // fuzzed sources still apply every patch).
    {
        let (out, ok) = dmp.patch_apply(&mut patches, &t1);
        assert!(ok.iter().all(|&x| x), "clean apply must apply all patches");
        assert_eq!(out.into_iter().collect::<String>(), t2);
    }
    let shifted = ds::shifted_source(&t1);
    {
        let (_, ok) = dmp.patch_apply(&mut patches, &shifted);
        assert!(
            ok.iter().all(|&x| x),
            "shifted apply must apply all patches"
        );
    }
    let fuzzed = ds::fuzz_patch_contexts(&t1, &patches);
    {
        let (_, ok) = dmp.patch_apply(&mut patches, &fuzzed);
        assert!(ok.iter().all(|&x| x), "fuzzed apply must apply all patches");
    }

    g.bench_function("apply_clean_50k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.patch_apply(black_box(&mut patches), black_box(&t1))
        })
    });
    g.bench_function("apply_shifted_50k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.patch_apply(black_box(&mut patches), black_box(&shifted))
        })
    });
    g.bench_function("apply_fuzzed_50k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.patch_apply(black_box(&mut patches), black_box(&fuzzed))
        })
    });
    g.finish();
}

fn wire(c: &mut Criterion) {
    let mut g = c.benchmark_group("wire");
    g.warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_millis(1_500))
        .sample_size(30);

    let (t1, t2) = ds::pair_scattered();
    let mut dmp = Dmp::new();
    let mut diffs = dmp.diff_main(&t1, &t2, true);
    let delta = dmp.diff_todelta(&mut diffs);
    let mut patches = dmp.patch_make1(&t1, &t2);
    let patch_text = dmp.patch_to_text(&mut patches);

    g.bench_function("todelta_scattered", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.diff_todelta(black_box(&mut diffs))
        })
    });
    g.bench_function("fromdelta_scattered", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.diff_from_delta(black_box(&t1), black_box(&delta))
        })
    });
    g.bench_function("patch_to_text_scattered", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.patch_to_text(black_box(&mut patches))
        })
    });
    g.bench_function("patch_from_text_scattered", |b| {
        b.iter_batched(
            || patch_text.clone(),
            |text| {
                let mut d = Dmp::new();
                d.patch_from_text(text)
            },
            BatchSize::SmallInput,
        )
    });
    g.finish();
}

fn match_(c: &mut Criterion) {
    let mut g = c.benchmark_group("match");
    g.warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_millis(1_500))
        .sample_size(50);

    let (hay, fuzzy_pattern, loc) = ds::match_case();
    {
        let mut d = Dmp::new();
        assert_ne!(
            d.match_main(&hay, &fuzzy_pattern, loc),
            -1,
            "bitap case must find its fuzzy match"
        );
    }
    g.bench_function("bitap_fuzzy_50k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.match_main(black_box(&hay), black_box(&fuzzy_pattern), black_box(loc))
        })
    });

    // Exact occurrence 300 chars past the expected location: the find_sub
    // speedup finds it, then bitap still scans with the tightened threshold.
    let exact_pattern = {
        let pos = (loc + 300) as usize;
        hay[pos..pos + 28].to_string()
    };
    let near_loc = loc;
    g.bench_function("exact_near_50k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.match_main(
                black_box(&hay),
                black_box(&exact_pattern),
                black_box(near_loc),
            )
        })
    });
    g.finish();
}

criterion_group!(
    benches,
    legacy,
    diff_realistic,
    diff_heavy,
    diff_pathological,
    cleanup,
    patch,
    wire,
    match_
);
criterion_main!(benches);
