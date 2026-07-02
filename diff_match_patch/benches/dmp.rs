use criterion::{criterion_group, criterion_main, Criterion};
use diff_match_patch::Dmp;

/// Multi-line texts sharing large common blocks between distinct lines —
/// exercises line mode, bisect, the cleanups, and patch construction.
fn interleaved_blocks(blocks: usize) -> (String, String) {
    let mut t1 = String::from("HEAD1\n");
    let mut t2 = String::from("HEAD2\n");
    for b in 0..blocks {
        for l in 0..8 {
            let line = format!("common-block{b}-line{l}\n");
            t1.push_str(&line);
            t2.push_str(&line);
        }
        t1.push_str(&format!("aaa-distinct-{b}\n"));
        t2.push_str(&format!("bbb-distinct-{b}\n"));
    }
    t1.push_str("TAIL1\n");
    t2.push_str("TAIL2\n");
    (t1, t2)
}

fn benches(c: &mut Criterion) {
    let (t1, t2) = interleaved_blocks(12);
    c.bench_function("diff_main/interleaved-2k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            d.diff_main(&t1, &t2, true)
        })
    });
    c.bench_function("patch_make+apply/interleaved-2k", |b| {
        b.iter(|| {
            let mut d = Dmp::new();
            let mut patches = d.patch_make1(&t1, &t2);
            d.patch_apply(&mut patches, &t1)
        })
    });
}

criterion_group!(bench_group, benches);
criterion_main!(bench_group);
