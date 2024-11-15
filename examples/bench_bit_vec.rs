/*
 * SPDX-FileCopyrightText: 2023 Inria
 * SPDX-FileCopyrightText: 2023 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */
use anyhow::Result;
use clap::Parser;
use criterion::{AxisScale, BenchmarkId, Criterion, PlotConfiguration};
use std::cmp::min;
use std::time::Duration;
use sux::prelude::*;

#[derive(Parser, Debug)]
#[command(about = "Benchmarks bit_vec", long_about = None)]
struct Args {
    #[arg(short, long, default_value = "10000")]
    start_min_len_iter: usize,
    #[arg(short, long, default_value = "100000")]
    stop_min_len_iter: usize,

    #[arg(short, long, default_value = "10000")]
    start_chunk_size: usize,
    #[arg(short, long, default_value = "100000")]
    stop_chunk_size: usize,

    #[arg(short, long, default_value = "1")]
    start_len: usize,
    #[arg(short, long, default_value = "1000000000")]
    stop_len: usize,

    #[arg(short, long, default_value = "5")]
    duration: usize,
}

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init()?;

    let mut args = Args::parse();
    args.stop_min_len_iter = min(args.stop_min_len_iter, args.stop_len);
    args.stop_chunk_size = min(args.stop_chunk_size, args.stop_len);

    use criterion::black_box;
    let mut c = Criterion::default()
        .with_output_color(true)
        .measurement_time(Duration::from_secs(args.duration as u64));
    let mut group = c.benchmark_group("fill");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    let mut len = args.start_len;
    while len <= args.stop_len {
        group.bench_with_input(BenchmarkId::new("default", len), &len, |b, _| {
            let mut vec = BitVec::new(len);
            b.iter(|| black_box(vec.fill(black_box(true))));
        });
        group.bench_with_input(BenchmarkId::new("no_rayon", len), &len, |b, _| {
            let mut vec = BitVec::new(len);
            b.iter(|| black_box(vec.fill_no_rayon(black_box(true))));
        });

        let mut min_len_iter = args.start_min_len_iter;
        while min_len_iter <= args.stop_min_len_iter {
            group.bench_with_input(
                BenchmarkId::new(format!("min_len_iter-{}", min_len_iter), len),
                &len,
                |b, _| {
                    let mut vec = BitVec::new(len);
                    b.iter(|| {
                        black_box(vec.fill_min_len_iter(black_box(true), black_box(min_len_iter)))
                    });
                },
            );
            min_len_iter *= 10;
        }

        let mut chunk_size = args.start_chunk_size;
        while chunk_size <= args.stop_chunk_size {
            group.bench_with_input(
                BenchmarkId::new(format!("chunk_size-{}", chunk_size), len),
                &len,
                |b, _| {
                    let mut vec = BitVec::new(len);
                    b.iter(|| black_box(vec.fill_chunks(black_box(true), black_box(chunk_size))));
                },
            );
            chunk_size *= 10;
        }

        len *= 10;
    }
    group.finish();
    c.final_summary();
    Ok(())
}
