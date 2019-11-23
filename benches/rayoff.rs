#![feature(test)]
extern crate rayon;
extern crate test;

use rayoff::Pool;
use rayon::prelude::*;

#[bench]
fn bench_pool(bencher: &mut Bencher) {
    let pool = Pool::new();
    bencher.iter(|| {
        let mut array = [0usize; 100];
        pool.dispach_mut(&array, |val| *val += 1);
        let expected = [1usize; 100];
        for i in 0..100 {
            assert_eq!(array[i], expected[i]);
        }
    })
}

#[bench]
fn bench_baseline(bencher: &mut Bencher) {
    bencher.iter(|| {
        let mut array = [0usize; 100];
        for i in array.iter_mut() {
            *i += 1;
        }
        let expected = [1usize; 100];
        for i in 0..100 {
            assert_eq!(array[i], expected[i]);
        }
    })
}

#[bench]
fn bench_rayon(bencher: &mut Bencher) {
    bencher.iter(|| {
        let mut array = [0usize; 100];
        array.par_iter_mut().for_each(|p| *p += 1);
        let expected = [1usize; 100];
        for i in 0..100 {
            assert_eq!(array[i], expected[i]);
        }
    })
}
