#![allow(missing_docs)]

use divan::{black_box, main, Bencher};

use eyeball::Observable;
use tokio::task::JoinSet;

#[divan::bench]
fn baseline(b: Bencher<'_, '_>) {
    b.with_inputs(|| Box::new([0; 256])).bench_refs(|x| {
        for i in 1..=256 {
            *x = black_box(Box::new([i; 256]));
        }
    });
}

#[divan::bench]
fn no_subscribers(b: Bencher<'_, '_>) {
    b.with_inputs(|| Observable::new(Box::new([0; 256]))).bench_refs(|ob| {
        for i in 1..=256 {
            Observable::set(ob, black_box(Box::new([i; 256])));
        }
    });
}

#[divan::bench(args = [1, 2, 4, 16, 64])]
fn n_subscribers(b: Bencher<'_, '_>, n: usize) {
    b.with_inputs(|| {
        let tokio_rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        let ob = Observable::new(Box::new([0; 256]));
        let mut join_set = JoinSet::new();
        for _ in 0..n {
            let mut subscriber = Observable::subscribe(&ob);
            tokio_rt.block_on(async {
                join_set.spawn(async move {
                    while let Some(value) = subscriber.next().await {
                        black_box(&value);
                    }
                });
            });
        }
        (tokio_rt, ob, join_set)
    })
    .bench_values(|(tokio_rt, mut ob, mut join_set)| {
        tokio_rt.block_on(async move {
            for i in 1..=256 {
                Observable::set(&mut ob, black_box(Box::new([i; 256])));
            }
            drop(ob);

            // wait for all tasks to finish
            while join_set.join_next().await.is_some() {}
        });
    });
}
