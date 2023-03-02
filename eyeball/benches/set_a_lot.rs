use std::sync::atomic::AtomicBool;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Bencher, Criterion};

use eyeball::Observable;
use tokio::task::JoinSet;

fn baseline(b: &mut Bencher<'_>) {
    let mut x = Box::new([0; 256]);
    b.iter(|| {
        for i in 1..=256 {
            black_box(&x);
            x = black_box(Box::new([i; 256]));
        }
    });
}

fn no_subscribers(b: &mut Bencher<'_>) {
    let mut ob = Observable::new(Box::new([0; 256]));
    b.iter(|| {
        for i in 1..=256 {
            black_box(&*ob);
            Observable::set(&mut ob, black_box(Box::new([i; 256])));
        }
    });
}

#[tokio::main]
async fn n_subscribers(n: usize, b: &mut Bencher<'_>) {
    static STOP: AtomicBool = AtomicBool::new(false);

    b.iter_batched(
        || {
            let ob = Observable::new(Box::new([0; 256]));
            let mut join_set = JoinSet::new();
            for _ in 0..n {
                let mut subscriber = Observable::subscribe(&ob);
                join_set.spawn(async move {
                    while let Some(value) = subscriber.next().await {
                        black_box(&value);
                    }
                });
            }
            (ob, join_set)
        },
        |(mut ob, mut join_set)| {
            for i in 1..=256 {
                Observable::set(&mut ob, black_box(Box::new([i; 256])));
            }
            drop(ob);
            tokio::task::block_in_place(|| {
                let tokio_handle = tokio::runtime::Handle::current();
                tokio_handle.block_on(async move {
                    // wait for all tasks to finish
                    while join_set.join_next().await.is_some() {}
                });
            });
        },
        BatchSize::LargeInput,
    );
}

fn set_a_lot(c: &mut Criterion) {
    c.bench_function("baseline", baseline);
    c.bench_function("no_subscribers", no_subscribers);
    c.bench_function("one_subscriber", |b| n_subscribers(1, b));
    c.bench_function("two_subscribers", |b| n_subscribers(2, b));
    c.bench_function("four_subscribers", |b| n_subscribers(4, b));
    c.bench_function("sixteen_subscribers", |b| n_subscribers(16, b));
    c.bench_function("sixtyfour_subscribers", |b| n_subscribers(64, b));
}

criterion_group!(benches, set_a_lot);
criterion_main!(benches);
