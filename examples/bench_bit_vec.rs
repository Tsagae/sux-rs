/*
 * SPDX-FileCopyrightText: 2023 Inria
 * SPDX-FileCopyrightText: 2023 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */
use anyhow::Result;
use clap::Parser;
use dsi_progress_logger::*;
use log::info;
use std::hint::black_box;
use std::time::SystemTime;
use sux::prelude::*;

#[derive(Parser, Debug)]
#[command(about = "Benchmarks bit_vec", long_about = None)]
struct Args {
    #[arg(short, long, default_value = "1")]
    start_min_len_iter: usize,
    #[arg(short, long, default_value = "1000000000")]
    stop_min_len_iter: usize,

    #[arg(short, long, default_value = "100000")]
    start_block_size: usize,
    #[arg(short, long, default_value = "1000000000")]
    stop_block_size: usize,

    #[arg(short, long, default_value = "1000000000")]
    len: usize,
    /// The number of test repetitions.
    #[arg(short, long, default_value = "500")]
    repeats: usize,
}

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init()?;

    let args = Args::parse();

    let mut a = BitVec::new(args.len);
    let mut pl = ProgressLogger::default();

    pl.start("Testing default rayon fill");
    let duration = repeat(|| black_box(a.fill(true)), args.repeats);
    pl.done_with_count(args.repeats);

    info!("avg: {}µs of {} runs\n", duration, args.repeats);

    pl.start("Testing no rayon fill");
    let duration = repeat(|| black_box(a.fill_no_rayon(true)), args.repeats);
    pl.done_with_count(args.repeats);
    info!("avg: {}µs of {} runs\n", duration, args.repeats);

    let mut min_len_iter = args.start_min_len_iter;
    while min_len_iter <= args.stop_min_len_iter {
        pl.start(format!("Testing min_len: {min_len_iter} fill"));
        let duration = repeat(
            || black_box(a.fill_min_len_iter(true, min_len_iter)),
            args.repeats,
        );
        pl.done_with_count(args.repeats);
        info!("avg: {}µs of {} runs\n", duration, args.repeats);
        min_len_iter *= 10;
    }

    let mut block_size = args.start_block_size;
    while block_size <= args.stop_block_size {
        pl.start(format!("Testing block size: {block_size} fill"));
        let duration = repeat(
            || black_box(a.fill_by_uniform_blocks(true, block_size)),
            args.repeats,
        );
        pl.done_with_count(args.repeats);
        info!("avg: {}µs of {} runs\n", duration, args.repeats);
        block_size *= 10;
    }

    let mut vec_size = 1;
    while vec_size <= args.len {
        a = BitVec::new(vec_size);
        info!("------------ vec size: {vec_size} ------------");

        pl.start(format!("Testing no rayon fill vec size: {vec_size}"));
        let duration = repeat(|| black_box(a.fill_no_rayon(true)), args.repeats);
        pl.done_with_count(args.repeats);
        info!("avg: {}µs of {} runs\n", duration, args.repeats);

        pl.start(format!("Testing rayon fill vec size: {vec_size}"));
        let duration = repeat(|| black_box(a.fill(true)), args.repeats);

        pl.done_with_count(args.repeats);
        info!("avg: {}µs of {} runs\n", duration, args.repeats);
        vec_size *= 10;
    }

    Ok(())
}

fn repeat(mut f: impl FnMut(), repeats: usize) -> f64 {
    let mut duration_sum = 0;
    for _ in 0..repeats {
        let start = SystemTime::now();
        f();
        let end = SystemTime::now();
        duration_sum += end.duration_since(start).unwrap().as_micros();
    }
    duration_sum as f64 / repeats as f64
}
