use std::{sync::RwLock, ops::Index};

use append_only_vec::AppendOnlyVec;
use scaling::{bench, bench_scaling_gen};

struct RwVec<T> {
    data: RwLock<Vec<T>>,
}

impl<T: Clone> RwVec<T> {
    fn new() -> Self {
        RwVec {
            data: RwLock::new(Vec::new())
        }
    }
    fn push(&self, val: T) {
        self.data.write().unwrap().push(val)
    }
    fn get(&self, index: usize) -> T {
        self.data.read().unwrap().index(index).clone()
    }
    fn len(&self) -> usize {
        self.data.read().unwrap().len()
    }
}

struct ParkVec<T> {
    data: parking_lot::RwLock<Vec<T>>,
}

impl<T: Clone> ParkVec<T> {
    fn new() -> Self {
        ParkVec {
            data: parking_lot::RwLock::new(Vec::new())
        }
    }
    fn push(&self, val: T) {
        self.data.write().push(val)
    }
    fn get(&self, index: usize) -> T {
        self.data.read().index(index).clone()
    }
    fn len(&self) -> usize {
        self.data.read().len()
    }
}

fn main() {
    {
        println!(
            "AOV: Filling 10 strings: {}",
            bench(|| {
                let v = AppendOnlyVec::new();
                for i in 0..10 {
                    v.push(format!("{}", i));
                }
                v
            })
        );
        println!(
            "RWV: Filling 10 strings: {}",
            bench(|| {
                let v = RwVec::new();
                for i in 0..10 {
                    v.push(format!("{}", i));
                }
                v
            })
        );
        println!(
            "plV: Filling 10 strings: {}",
            bench(|| {
                let v = ParkVec::new();
                for i in 0..10 {
                    v.push(format!("{}", i));
                }
                v
            })
        );
        println!(
            "Vec: Filling 10 strings: {}",
            bench(|| {
                let mut v = Vec::new();
                for i in 0..10 {
                    v.push(format!("{}", i));
                }
                v
            })
        );

        println!();

        println!(
            "AOV: Filling 100 strings: {}",
            bench(|| {
                let v = AppendOnlyVec::new();
                for i in 0..100 {
                    v.push(format!("{}", i));
                }
                v
            })
        );
        println!(
            "RWV: Filling 100 strings: {}",
            bench(|| {
                let v = RwVec::new();
                for i in 0..100 {
                    v.push(format!("{}", i));
                }
                v
            })
        );
        println!(
            "plV: Filling 100 strings: {}",
            bench(|| {
                let v = ParkVec::new();
                for i in 0..100 {
                    v.push(format!("{}", i));
                }
                v
            })
        );
        println!(
            "Vec: Filling 100 strings: {}",
            bench(|| {
                let mut v = Vec::new();
                for i in 0..100 {
                    v.push(format!("{}", i));
                }
                v
            })
        );
    }
    println!();
    {
        let min_n = 1000;
        println!(
            "AOV: sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let vec = AppendOnlyVec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| { vec.iter().copied().sum::<usize>() },
                min_n
            )
        );
        println!(
            "AOV: reversed sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let vec = AppendOnlyVec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| { vec.iter().copied().rev().sum::<usize>() },
                min_n
            )
        );
        println!(
            "Vec: sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let mut vec = Vec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| { vec.iter().copied().sum::<usize>() },
                min_n
            )
        );

        println!();

        println!(
            "AOV: loop sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let vec = AppendOnlyVec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| {
                    let mut sum = 0;
                    for i in 0..vec.len() {
                        sum += vec[i];
                    }
                    sum
                },
                min_n
            )
        );
        println!(
            "RWV: loop sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let vec = RwVec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| {
                    let mut sum = 0;
                    for i in 0..vec.len() {
                        sum += vec.get(i);
                    }
                    sum
                },
                min_n
            )
        );
        println!(
            "plV: loop sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let vec = ParkVec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| {
                    let mut sum = 0;
                    for i in 0..vec.len() {
                        sum += vec.get(i);
                    }
                    sum
                },
                min_n
            )
        );
        println!(
            "Vec: loop sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let mut vec = Vec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| {
                    let mut sum = 0;
                    for i in 0..vec.len() {
                        sum += vec[i];
                    }
                    sum
                },
                min_n
            )
        );

        println!();

        println!(
            "AOV: back loop sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let vec = AppendOnlyVec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| {
                    let mut sum = 0;
                    let n = vec.len();
                    for i in 0..n {
                        sum += vec[n-1-i];
                    }
                    sum
                },
                min_n
            )
        );
        println!(
            "RWV: back loop sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let vec = RwVec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| {
                    let mut sum = 0;
                    let n = vec.len();
                    for i in 0..n {
                        sum += vec.get(n-1-i);
                    }
                    sum
                },
                min_n
            )
        );
        println!(
            "plV: back loop sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let vec = ParkVec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| {
                    let mut sum = 0;
                    let n = vec.len();
                    for i in 0..n {
                        sum += vec.get(n-1-i);
                    }
                    sum
                },
                min_n
            )
        );
        println!(
            "Vec: back loop sum: {}",
            bench_scaling_gen(
                |n: usize| {
                    let mut vec = Vec::new();
                    for i in 0..n {
                        vec.push(i);
                    }
                    vec
                },
                |vec| {
                    let mut sum = 0;
                    let n = vec.len();
                    for i in 0..n {
                        sum += vec[n-1-i];
                    }
                    sum
                },
                min_n
            )
        );
    }
    println!();
}
