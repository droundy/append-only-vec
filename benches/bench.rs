use append_only_vec::AppendOnlyVec;
use scaling::{bench, bench_scaling_gen};

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
            "Vec: Filling 10 strings: {}",
            bench(|| {
                let mut v = Vec::new();
                for i in 0..10 {
                    v.push(format!("{}", i));
                }
                v
            })
        );
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
