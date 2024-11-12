/*
 * SPDX-FileCopyrightText: 2023 Inria
 * SPDX-FileCopyrightText: 2023 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */
use anyhow::Result;
use clap::Parser;
use dsi_progress_logger::*;
use std::fmt::Display;
use std::hint::black_box;
use std::time::SystemTime;
use sux::prelude::*;


#[derive(Parser, Debug)]
#[command(about = "Benchmarks bit_vec", long_about = None)]
struct Args {
    #[arg(short, long, default_value = "100000")]
    min_len_iter: usize,
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

    let mut iter_size: usize = 1;
    while iter_size <= args.min_len_iter {
        let mut duration_sum = 0;
        pl.start(&format!("Testing iter size {iter_size} fill"));
        for _ in 0..args.repeats {
            let start = SystemTime::now();
            black_box(a.fill_min_len_iter(true, iter_size));
            let end = SystemTime::now();
            duration_sum += end.duration_since(start)?.as_micros();
        }
        pl.done_with_count(args.repeats);
        eprintln!("avg: {}µs of {} runs", duration_sum as f64 / args.repeats as f64, args.repeats);
        iter_size *= 10;
    }

    Ok(())
}
