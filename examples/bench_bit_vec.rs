/*
 * SPDX-FileCopyrightText: 2023 Inria
 * SPDX-FileCopyrightText: 2023 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */
use anyhow::Result;
use clap::Parser;
use dsi_progress_logger::*;
use std::hint::black_box;
use std::time::SystemTime;
use sux::prelude::*;


#[derive(Parser, Debug)]
#[command(about = "Benchmarks bit_vec", long_about = None)]
struct Args {
    #[arg(short, long, default_value = "1")]
    start_min_len_iter: usize,
    #[arg(short, long, default_value = "100000")]
    stop_min_len_iter: usize,

    #[arg(short, long, default_value = "10000")]
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

    let mut duration_sum = 0;
    pl.start("Testing no rayon fill");
    for _ in 0..args.repeats {
        let start = SystemTime::now();
        black_box(a.fill_no_rayon(true));
        let end = SystemTime::now();
        duration_sum += end.duration_since(start)?.as_micros();
    }
    pl.done_with_count(args.repeats);
    eprintln!("avg: {}µs of {} runs", duration_sum as f64 / args.repeats as f64, args.repeats);

    let mut min_len_iter= args.start_min_len_iter;
    while min_len_iter <= args.stop_min_len_iter {
        let mut duration_sum = 0;
        pl.start(&format!("Testing iter size {min_len_iter} fill"));
        for _ in 0..args.repeats {
            let start = SystemTime::now();
            black_box(a.fill_min_len_iter(true, min_len_iter));
            let end = SystemTime::now();
            duration_sum += end.duration_since(start)?.as_micros();
        }
        pl.done_with_count(args.repeats);
        eprintln!("avg: {}µs of {} runs", duration_sum as f64 / args.repeats as f64, args.repeats);
        min_len_iter *= 10;
    }

    let mut block_size = args.start_block_size;
    while block_size <= args.stop_block_size {
        let mut duration_sum = 0;
        pl.start(&format!("Testing iter size {block_size} fill"));
        for _ in 0..args.repeats {
            let start = SystemTime::now();
            black_box(a.fill_by_uniform_blocks(true, block_size));
            let end = SystemTime::now();
            duration_sum += end.duration_since(start)?.as_micros();
        }
        pl.done_with_count(args.repeats);
        eprintln!("avg: {}µs of {} runs", duration_sum as f64 / args.repeats as f64, args.repeats);
        block_size *= 10;
    }

    Ok(())
}
