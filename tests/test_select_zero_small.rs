/*
 * SPDX-FileCopyrightText: 2024 Michele Andreata
 * SPDX-FileCopyrightText: 2024 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */

use rand::{rngs::SmallRng, Rng, SeedableRng};
use sux::prelude::{BitCount, BitLength, BitVec, RankSmall, SelectZero, SelectZeroSmall};

macro_rules! test_rank_small_sel_zero {
    ($NUM_U32S: literal; $COUNTER_WIDTH: literal; $LOG2_ZEROS_PER_INVENTORY: literal) => {
        let mut rng = SmallRng::seed_from_u64(0);
        let density = 0.5;
        let lens = (1..1000)
            .chain((1000..10000).step_by(100))
            .chain([1 << 20, 1 << 24]);
        for len in lens {
            let bits = (0..len).map(|_| rng.gen_bool(density)).collect::<BitVec>();
            let rank_small_sel =
                SelectZeroSmall::<$NUM_U32S, $COUNTER_WIDTH, $LOG2_ZEROS_PER_INVENTORY, _>::new(
                    RankSmall::<$NUM_U32S, $COUNTER_WIDTH, _>::new(bits.clone()),
                );

            let zeros = bits.len() - bits.count_ones();
            let mut pos = Vec::with_capacity(zeros);
            for i in 0..len {
                if !bits[i] {
                    pos.push(i);
                }
            }

            for i in 0..zeros {
                assert_eq!(rank_small_sel.select_zero(i), Some(pos[i]));
            }
            assert_eq!(rank_small_sel.select_zero(zeros + 1), None);
        }
    };
}

#[test]
fn test_rank_small_sel_zero0() {
    test_rank_small_sel_zero!(2; 9; 13);
}

#[test]
fn test_rank_small_sel_zero1() {
    test_rank_small_sel_zero!(1; 9; 13);
}

#[test]
fn test_rank_small_sel_zero2() {
    test_rank_small_sel_zero!(1; 10; 13);
}

#[test]
fn test_rank_small_sel_zero3() {
    test_rank_small_sel_zero!(1; 11; 13);
}

#[test]
fn test_rank_small_sel_zero4() {
    test_rank_small_sel_zero!(3; 13; 13);
}

#[test]
fn test_empty() {
    let bits = BitVec::new(0);
    let select = SelectZeroSmall::<2, 9>::new(RankSmall::<2, 9>::new(bits.clone()));
    assert_eq!(select.count_ones(), 0);
    assert_eq!(select.len(), 0);
    assert_eq!(select.select_zero(0), None);

    let inner = select.into_inner();
    assert_eq!(inner.len(), 0);
    let inner = inner.into_inner();
    assert_eq!(inner.len(), 0);
}

#[test]
fn test_ones() {
    let len = 300_000;
    let bits = (0..len).map(|_| true).collect::<BitVec>();
    let select = SelectZeroSmall::<2, 9>::new(RankSmall::<2, 9>::new(bits));

    assert_eq!(select.len(), len);
    assert_eq!(select.select_zero(0), None);
}

#[test]
fn test_zeros() {
    let len = 300_000;
    let bits = (0..len).map(|_| false).collect::<BitVec>().into();
    let select = SelectZeroSmall::<2, 9>::new(RankSmall::<2, 9>::new(bits));
    assert_eq!(select.len(), len);
    for i in 0..len {
        assert_eq!(select.select_zero(i), Some(i));
    }
}

#[test]
fn test_few_zeros() {
    let lens = [1 << 18, 1 << 19, 1 << 20];
    for len in lens {
        for num_ones in [1, 2, 4, 8, 16, 32, 64, 128] {
            let bits = (0..len)
                .map(|i| i % (len / num_ones) != 0)
                .collect::<BitVec>();
            let select = SelectZeroSmall::<2, 9>::new(RankSmall::<2, 9>::new(bits));
            assert_eq!(select.len(), len);
            for i in 0..num_ones {
                assert_eq!(select.select_zero(i), Some(i * (len / num_ones)));
            }
        }
    }
}

#[test]
fn test_select_adapt_non_uniform() {
    let lens = [1 << 18, 1 << 19, 1 << 20];

    let mut rng = SmallRng::seed_from_u64(0);
    for len in lens {
        for density in [0.5] {
            let density0 = density * 0.01;
            let density1 = density * 0.99;

            let len1;
            let len2;
            if len % 2 != 0 {
                len1 = len / 2 + 1;
                len2 = len / 2;
            } else {
                len1 = len / 2;
                len2 = len / 2;
            }

            let first_half = loop {
                let b = (0..len1)
                    .map(|_| rng.gen_bool(density0))
                    .collect::<BitVec>();
                if b.count_ones() > 0 {
                    break b;
                }
            };
            let num_ones_first_half = first_half.count_ones();
            let second_half = (0..len2)
                .map(|_| rng.gen_bool(density1))
                .collect::<BitVec>();
            let num_ones_second_half = second_half.count_ones();

            assert!(num_ones_first_half > 0);
            assert!(num_ones_second_half > 0);

            let bits = first_half
                .into_iter()
                .chain(second_half.into_iter())
                .collect::<BitVec>();

            assert_eq!(
                num_ones_first_half + num_ones_second_half,
                bits.count_ones()
            );

            assert_eq!(bits.len(), len as usize);

            let zeros = bits.len() - bits.count_ones();
            let mut pos = Vec::with_capacity(zeros);
            for i in 0..(len as usize) {
                if !bits[i] {
                    pos.push(i);
                }
            }

            let select = SelectZeroSmall::<2, 9>::new(RankSmall::<2, 9>::new(bits));
            for i in 0..zeros {
                assert_eq!(select.select_zero(i), Some(pos[i]));
            }
            assert_eq!(select.select_zero(zeros + 1), None);
        }
    }
}
