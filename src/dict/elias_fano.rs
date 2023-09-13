/*
 *
 * SPDX-FileCopyrightText: 2023 Tommaso Fontana
 * SPDX-FileCopyrightText: 2023 Inria
 * SPDX-FileCopyrightText: 2023 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */

/*!

Implementation of the Elias--Fano representation of monotone sequences.

There are two ways to build an [`EliasFano`] structure: using
an [`EliasFanoBuilder`] or an [`EliasFanoAtomicBuilder`].

The main trait implemented by [`EliasFano`] is [`IndexedDict`], which
makes it possible to access its values with [`IndexedDict::get`].

 */
use crate::prelude::*;
use anyhow::{bail, Result};
use core::sync::atomic::{AtomicUsize, Ordering};
use epserde::*;

/// The default combination of parameters return by the builders
pub type DefaultEliasFano = EliasFano<CountBitVec, CompactArray>;

/// A sequential builder for [`EliasFano`].
///
/// After creating an instance, you can use [`EliasFanoBuilder::push`] to add new values.
pub struct EliasFanoBuilder {
    u: usize,
    n: usize,
    l: usize,
    low_bits: CompactArray<Vec<usize>>,
    high_bits: BitVec<Vec<usize>>,
    last_value: usize,
    count: usize,
}

impl EliasFanoBuilder {
    /// Create a builder for an [`EliasFano`] containing
    /// `n` numbers smaller than `u`.
    pub fn new(n: usize, u: usize) -> Self {
        let l = if u >= n {
            (u as f64 / n as f64).log2().floor() as usize
        } else {
            0
        };

        Self {
            u,
            n,
            l,
            low_bits: CompactArray::new(l, n),
            high_bits: BitVec::new(n + (u >> l) + 1),
            last_value: 0,
            count: 0,
        }
    }

    /// Add a new value to the builder.
    ///
    /// # Panic
    /// May panic if the value is smaller than the last provided
    /// value, or if too many values are provided.
    pub fn push(&mut self, value: usize) -> Result<()> {
        if self.count == self.n {
            bail!("Too many values");
        }
        if value >= self.u {
            bail!("Value too large: {} >= {}", value, self.u);
        }
        if value < self.last_value {
            bail!("The values given to elias-fano are not monotone");
        }
        unsafe {
            self.push_unchecked(value);
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Values passed to this function must be smaller than `u` and must be monotone.
    /// Moreover, the function should not be called more than `n` times.
    pub unsafe fn push_unchecked(&mut self, value: usize) {
        let low = value & ((1 << self.l) - 1);
        self.low_bits.set(self.count, low);

        let high = (value >> self.l) + self.count;
        self.high_bits.set(high, true);

        self.count += 1;
        self.last_value = value;
    }

    pub fn build(self) -> DefaultEliasFano {
        EliasFano {
            u: self.u,
            n: self.n,
            l: self.l,
            low_bits: self.low_bits,
            high_bits: self.high_bits.with_count(self.n),
        }
    }
}

/// A parallel builder for [`EliasFano`].
///
/// After creating an instance, you can use [`EliasFanoAtomicBuilder::set`]
/// to set the values concurrently. However, this operation is inherently
/// unsafe as no check is performed on the provided data (e.g., duplicate
/// indices and lack of monotonicity are not detected).
pub struct EliasFanoAtomicBuilder {
    u: usize,
    n: usize,
    l: usize,
    low_bits: CompactArray<Vec<AtomicUsize>>,
    high_bits: BitVec<Vec<AtomicUsize>>,
}

impl EliasFanoAtomicBuilder {
    /// Create a builder for an [`EliasFano`] containing
    /// `n` numbers smaller than `u`.
    pub fn new(n: usize, u: usize) -> Self {
        let l = if u >= n {
            (u as f64 / n as f64).log2().floor() as usize
        } else {
            0
        };

        Self {
            u,
            n,
            l,
            low_bits: CompactArray::new_atomic(l, n),
            high_bits: BitVec::new_atomic(n + (u >> l) + 1),
        }
    }

    /// Concurrently set values.
    ///
    /// # Safety
    /// - All indices must be distinct.
    /// - All values must be smaller than `u`.
    /// - All indices must be smaller than `n`.
    /// - You must call this function exactly `n` times.
    pub unsafe fn set(&self, index: usize, value: usize, order: Ordering) {
        let low = value & ((1 << self.l) - 1);
        // Note that the concurrency guarantees of CompactArray
        // are sufficient for us.
        self.low_bits.set_unchecked(index, low, order);

        let high = (value >> self.l) + index;
        self.high_bits.set(high, true, order);
    }

    pub fn build(self) -> DefaultEliasFano {
        let bit_vec: BitVec<Vec<usize>> = self.high_bits.into();
        EliasFano {
            u: self.u,
            n: self.n,
            l: self.l,
            low_bits: self.low_bits.into(),
            high_bits: bit_vec.with_count(self.n),
        }
    }
}

#[derive(Epserde, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EliasFano<H, L> {
    /// An upper bound to the values.
    u: usize,
    /// The number of values.
    n: usize,
    /// The number of lower bits.
    l: usize,
    /// The lower-bits array.
    low_bits: L,
    /// the higher-bits array.
    high_bits: H,
}

impl<H, L> EliasFano<H, L> {
    #[inline]
    pub fn len(&self) -> usize {
        self.n
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Estimate the size of an instance.
    pub fn estimate_size(u: usize, n: usize) -> usize {
        2 * n + (n * (u as f64 / n as f64).log2().ceil() as usize)
    }

    pub fn transform<F, H2, L2>(self, func: F) -> EliasFano<H2, L2>
    where
        F: Fn(H, L) -> (H2, L2),
    {
        let (high_bits, low_bits) = func(self.high_bits, self.low_bits);
        EliasFano {
            u: self.u,
            n: self.n,
            l: self.l,
            low_bits,
            high_bits,
        }
    }
}

/**
Implementation of the Elias--Fano representation of monotone sequences.

There are two ways to build an [`EliasFano`] structure: using
an [`EliasFanoBuilder`] or an [`EliasFanoAtomicBuilder`].

Once the structure has been built, it is possible to enrich it with
indices that will make operations faster. This is done by calling
[ConvertTo::convert_to] towards the desired type. For example,
```rust
use sux::prelude::*;
let mut efb = EliasFanoBuilder::new(2, 5);
efb.push(0);
efb.push(1);
let ef = efb.build();
// Add an index on the ones (accelerates get operations).
let efo: EliasFano<QuantumIndex<CountBitVec>, CompactArray> =
    ef.convert_to().unwrap();
// Add also an index on the zeros  (accelerates precedessor and successor).
let efoz: EliasFano<QuantumZeroIndex<QuantumIndex<CountBitVec>>, CompactArray> =
    efo.convert_to().unwrap();
```

The main trait implemented is [`IndexedDict`], which
makes it possible to access values with [`IndexedDict::get`].
 */
impl<H, L> EliasFano<H, L> {
    /// # Safety
    /// No check is performed.
    #[inline(always)]
    pub unsafe fn from_raw_parts(u: usize, n: usize, l: usize, low_bits: L, high_bits: H) -> Self {
        Self {
            u,
            n,
            l,
            low_bits,
            high_bits,
        }
    }
    #[inline(always)]
    pub fn into_raw_parts(self) -> (usize, usize, usize, L, H) {
        (self.u, self.n, self.l, self.low_bits, self.high_bits)
    }
}

impl<H: Select, L: VSlice> IndexedDict for EliasFano<H, L> {
    type Value = usize;
    #[inline]
    fn len(&self) -> usize {
        self.n
    }

    #[inline(always)]
    unsafe fn get_unchecked(&self, index: usize) -> usize {
        let high_bits = self.high_bits.select_unchecked(index) - index;
        let low_bits = self.low_bits.get_unchecked(index);
        (high_bits << self.l) | low_bits
    }
}

impl<H1, L1, H2, L2> ConvertTo<EliasFano<H1, L1>> for EliasFano<H2, L2>
where
    H2: ConvertTo<H1>,
    L2: ConvertTo<L1>,
{
    #[inline(always)]
    fn convert_to(self) -> Result<EliasFano<H1, L1>> {
        Ok(EliasFano {
            u: self.u,
            n: self.n,
            l: self.l,
            low_bits: self.low_bits.convert_to()?,
            high_bits: self.high_bits.convert_to()?,
        })
    }
}
