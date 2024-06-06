/*
 *
 * SPDX-FileCopyrightText: 2024 Michele Andreata
 * SPDX-FileCopyrightText: 2023 Tommaso Fontana
 * SPDX-FileCopyrightText: 2024 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */

use crate::traits::*;
use common_traits::SelectInWord;
use epserde::*;
use mem_dbg::*;

/// A constant-based version of [`SimpleSelect`].
///
/// [`SimpleSelectConst`] has almost the same implementation of
/// [`SimpleSelect`](super::SimpleSelect), with two important differences:
///
/// - The number of ones per inventory and the number of u64s per subinventory
///   are fixed at compile time.
/// - There is no spill vector, so the space occupancy is fixed.
///
/// Because of these simplifications, [`SimpleSelectConst`] is slightly faster
/// than [`SimpleSelect`](super::SimpleSelect). However, since the number of
/// ones per inventory of a [`SimpleSelect`](super::SimpleSelect) is chosen
/// depending on the density of the bit vector, [`SimpleSelectConst`] can be
/// used only when the density of the bit vector is known in advance. A typical
/// use case is the [Elias–Fano representation of monotone
/// sequences](crate::dict::elias_fano::EliasFano), where the high bits have
/// density approximately 1/2.
///
/// # Examples
/// ```rust
/// use sux::bit_vec;
/// use sux::traits::{Rank, Select};
/// use sux::rank_sel::{SimpleSelectConst, Rank9};
///
/// // Standalone select
/// let bits = bit_vec![1, 0, 1, 1, 0, 1, 0, 1];
/// let select = SimpleSelectConst::<_, _, 8, 2>::new(bits.clone());
///
/// assert_eq!(select.select(0), Some(0));
/// assert_eq!(select.select(1), Some(2));
/// assert_eq!(select.select(2), Some(3));
/// assert_eq!(select.select(3), Some(5));
/// assert_eq!(select.select(4), Some(7));
/// assert_eq!(select.select(5), None);
///
/// // Select over a Rank9 structure, with alternative constants
/// // (256 ones per inventory, subinventories made of 4 64-bit words).
/// let rank9 = Rank9::new(bits);
/// let rank_sel = SimpleSelectConst::<_, _, 8, 2>::new(rank9);
///
/// assert_eq!(rank_sel.rank(0), 0);
/// assert_eq!(rank_sel.rank(1), 1);
/// assert_eq!(rank_sel.rank(2), 1);
/// assert_eq!(rank_sel.rank(3), 2);
/// assert_eq!(rank_sel.rank(4), 3);
/// assert_eq!(rank_sel.rank(5), 3);
/// assert_eq!(rank_sel.rank(6), 4);
/// assert_eq!(rank_sel.rank(7), 4);
/// assert_eq!(rank_sel.rank(8), 5);
///
/// assert_eq!(rank_sel.select(0), Some(0));
/// assert_eq!(rank_sel.select(1), Some(2));
/// assert_eq!(rank_sel.select(2), Some(3));
/// assert_eq!(rank_sel.select(3), Some(5));
/// assert_eq!(rank_sel.select(4), Some(7));
/// assert_eq!(rank_sel.select(5), None);
/// ```

#[derive(Epserde, Debug, Clone, MemDbg, MemSize)]
pub struct SimpleSelectConst<
    B,
    I,
    const LOG2_ONES_PER_INVENTORY: usize = 10,
    const LOG2_U64_PER_SUBINVENTORY: usize = 2,
> {
    bits: B,
    inventory: I,
}

/// constants used throughout the code
impl<B, I, const LOG2_ONES_PER_INVENTORY: usize, const LOG2_U64_PER_SUBINVENTORY: usize>
    SimpleSelectConst<B, I, LOG2_ONES_PER_INVENTORY, LOG2_U64_PER_SUBINVENTORY>
{
    const ONES_PER_INVENTORY: usize = 1 << LOG2_ONES_PER_INVENTORY;
    const U64_PER_SUBINVENTORY: usize = 1 << LOG2_U64_PER_SUBINVENTORY;
    const U64_PER_INVENTORY: usize = 1 + Self::U64_PER_SUBINVENTORY;

    const LOG2_ONES_PER_SUB64: usize = LOG2_ONES_PER_INVENTORY - LOG2_U64_PER_SUBINVENTORY;
    const ONES_PER_SUB64: usize = 1 << Self::LOG2_ONES_PER_SUB64;

    const LOG2_ONES_PER_SUB16: usize = Self::LOG2_ONES_PER_SUB64 - 2;
    const ONES_PER_SUB16: usize = 1 << Self::LOG2_ONES_PER_SUB16;

    /// We use the sign bit to store the type of the subinventory (u16 vs. usize).
    const INVENTORY_MASK: usize = (1 << 63) - 1;

    /// Return the inner BitVector
    pub fn into_inner(self) -> B {
        self.bits
    }

    /// Return raw bits and inventory
    pub fn into_raw_parts(self) -> (B, I) {
        (self.bits, self.inventory)
    }

    /// Create a new instance from raw bits and inventory
    ///
    /// # Safety
    /// The inventory must be consistent with the bits otherwise you will get
    /// wrong results, and possibly memory corruption.
    pub unsafe fn from_raw_parts(bits: B, inventory: I) -> Self {
        SimpleSelectConst { bits, inventory }
    }

    /// Modify the inner BitVector with possibly another type
    pub fn map_bits<B2>(
        self,
        f: impl FnOnce(B) -> B2,
    ) -> SimpleSelectConst<B2, I, LOG2_ONES_PER_INVENTORY, LOG2_U64_PER_SUBINVENTORY>
    where
        B2: SelectHinted,
    {
        SimpleSelectConst {
            bits: f(self.bits),
            inventory: self.inventory,
        }
    }

    /// Modify the inner inventory with possibly another type
    pub fn map_inventory<I2>(
        self,
        f: impl FnOnce(I) -> I2,
    ) -> SimpleSelectConst<B, I2, LOG2_ONES_PER_INVENTORY, LOG2_U64_PER_SUBINVENTORY>
    where
        I2: AsRef<[usize]>,
    {
        SimpleSelectConst {
            bits: self.bits,
            inventory: f(self.inventory),
        }
    }

    /// Modify the inner BitVector and inventory with possibly other types
    pub fn map<B2, I2>(
        self,
        f: impl FnOnce(B, I) -> (B2, I2),
    ) -> SimpleSelectConst<B2, I2, LOG2_ONES_PER_INVENTORY, LOG2_U64_PER_SUBINVENTORY>
    where
        B2: SelectHinted,
        I2: AsRef<[usize]>,
    {
        let (bits, inventory) = f(self.bits, self.inventory);
        SimpleSelectConst { bits, inventory }
    }
}

impl<
        B: SelectHinted + BitLength + BitCount + AsRef<[usize]>,
        const LOG2_ONES_PER_INVENTORY: usize,
        const LOG2_U64_PER_SUBINVENTORY: usize,
    > SimpleSelectConst<B, Vec<usize>, LOG2_ONES_PER_INVENTORY, LOG2_U64_PER_SUBINVENTORY>
{
    pub fn new(bitvec: B) -> Self {
        // number of inventories we will create
        let inventory_size = bitvec.count_ones().div_ceil(Self::ONES_PER_INVENTORY);
        let inventory_len = inventory_size * Self::U64_PER_INVENTORY + 1;
        // inventory_size, an usize for the first layer index, and Self::U64_PER_SUBINVENTORY for the sub layer
        let mut inventory = Vec::with_capacity(inventory_len);
        // scan the bitvec and fill the first layer of the inventory
        let mut number_of_ones = 0;
        let mut next_quantum = 0;
        for (i, word) in bitvec.as_ref().iter().copied().enumerate() {
            let ones_in_word = word.count_ones() as usize;
            // skip the word if we can
            while number_of_ones + ones_in_word > next_quantum {
                let in_word_index = word.select_in_word(next_quantum - number_of_ones);
                let index = (i * usize::BITS as usize) + in_word_index;

                // write the position of the one in the inventory
                inventory.push(index);
                // make space for the subinventory
                inventory.resize(inventory.len() + Self::U64_PER_SUBINVENTORY, 0);

                next_quantum += Self::ONES_PER_INVENTORY;
            }
            number_of_ones += ones_in_word;
        }
        // in the last inventory write the number of bits
        inventory.push(BitLength::len(&bitvec));
        assert_eq!(inventory_len, inventory.len());
        // build the index
        let iter = 0..inventory_size;

        // fill the second layer of the index
        iter.for_each(|inventory_idx| {
            // get the start and end usize index of the current inventory
            let start_idx = inventory_idx * Self::U64_PER_INVENTORY;
            let end_idx = start_idx + Self::U64_PER_INVENTORY;
            // read the first level index to get the start and end bit index
            let start_bit_idx = inventory[start_idx];
            let end_bit_idx = inventory[end_idx];
            // compute the span of the inventory
            let span = end_bit_idx - start_bit_idx;
            // compute were we should the word boundaries of where we should
            // scan
            let mut word_idx = start_bit_idx / usize::BITS as usize;

            // cleanup the lower bits
            let bit_idx = start_bit_idx % usize::BITS as usize;
            let mut word = (bitvec.as_ref()[word_idx] >> bit_idx) << bit_idx;
            // compute the global number of ones
            let mut number_of_ones = inventory_idx * Self::ONES_PER_INVENTORY;
            // and what's the next bit rank which index should log in the sub
            // inventory (the first subinventory element is always 0)
            let mut next_quantum = number_of_ones;
            let quantum;

            if span <= u16::MAX as usize {
                quantum = Self::ONES_PER_SUB16;
            } else {
                quantum = Self::ONES_PER_SUB64;
                inventory[start_idx] |= 1_usize << 63;
            }

            let end_word_idx = end_bit_idx.div_ceil(usize::BITS as usize);

            // the first subinventory element is always 0
            let mut subinventory_idx = 1;
            next_quantum += quantum;

            'outer: loop {
                let ones_in_word = word.count_ones() as usize;

                // if the quantum is in this word, write it in the subinventory
                // this can happen multiple times if the quantum is small
                while number_of_ones + ones_in_word > next_quantum {
                    debug_assert!(next_quantum <= end_bit_idx as _);
                    // find the quantum bit in the word
                    let in_word_index = word.select_in_word(next_quantum - number_of_ones);
                    // compute the global index of the quantum bit in the bitvec
                    let bit_index = (word_idx * usize::BITS as usize) + in_word_index;
                    // compute the offset of the quantum bit
                    // from the start of the subinventory
                    let sub_offset = bit_index - start_bit_idx;

                    if span <= u16::MAX as usize {
                        let subinventory: &mut [u16] =
                            unsafe { inventory[start_idx + 1..end_idx].align_to_mut().1 };

                        subinventory[subinventory_idx] = sub_offset as u16;
                    } else {
                        inventory[start_idx + 1 + subinventory_idx] = sub_offset;
                    }

                    // update the subinventory index and the next quantum
                    subinventory_idx += 1;
                    if subinventory_idx == (1 << LOG2_ONES_PER_INVENTORY) / quantum {
                        break 'outer;
                    }

                    next_quantum += quantum;
                }

                // we are done with the word, so update the number of ones
                number_of_ones += ones_in_word;
                // move to the next word and boundcheck
                word_idx += 1;
                if word_idx == end_word_idx {
                    break;
                }

                // read the next word
                word = bitvec.as_ref()[word_idx];
            }
        });

        Self {
            bits: bitvec,
            inventory,
        }
    }
}

/// Provide the hint to the underlying structure
impl<
        B: SelectHinted + BitCount,
        I: AsRef<[usize]>,
        const LOG2_ONES_PER_INVENTORY: usize,
        const LOG2_U64_PER_SUBINVENTORY: usize,
    > Select for SimpleSelectConst<B, I, LOG2_ONES_PER_INVENTORY, LOG2_U64_PER_SUBINVENTORY>
{
    #[inline(always)]
    unsafe fn select_unchecked(&self, rank: usize) -> usize {
        // find the index of the first level inventory
        let inventory_index = rank / Self::ONES_PER_INVENTORY;
        // find the index of the second level inventory
        let subrank = rank % Self::ONES_PER_INVENTORY;
        // find the position of the first index value in the interleaved inventory
        let start_idx = inventory_index * (1 + Self::U64_PER_SUBINVENTORY);
        // read the potentially unaliged i64 (i.e. the first index value)
        let inventory_rank = *self.inventory.as_ref().get_unchecked(start_idx);
        // get a reference to the u64s in this subinventory
        let u64s = self
            .inventory
            .as_ref()
            .get_unchecked(start_idx + 1..start_idx + 1 + Self::U64_PER_SUBINVENTORY);

        // if the inventory_rank is positive, the subranks are u16s otherwise they are u64s
        let (pos, residual) = if inventory_rank as isize >= 0 {
            let (_pre, u16s, _post) = u64s.align_to::<u16>();
            (
                inventory_rank + *u16s.get_unchecked(subrank / Self::ONES_PER_SUB16) as usize,
                subrank % Self::ONES_PER_SUB16,
            )
        } else {
            (
                (inventory_rank & Self::INVENTORY_MASK)
                    + u64s.get_unchecked(subrank / Self::ONES_PER_SUB64),
                subrank % Self::ONES_PER_SUB64,
            )
        };

        // linear scan to finish the search
        self.bits
            .select_hinted_unchecked(rank, pos, rank - residual)
    }
}

crate::forward_mult![
    SimpleSelectConst<B, I, [const] LOG2_ONES_PER_INVENTORY: usize, [const] LOG2_U64_PER_SUBINVENTORY: usize>; B; bits;
    crate::forward_as_ref_slice_usize,
    crate::forward_index_bool,
    crate::traits::rank_sel::forward_bit_length,
    crate::traits::rank_sel::forward_bit_count,
    crate::traits::rank_sel::forward_rank,
    crate::traits::rank_sel::forward_rank_hinted,
    crate::traits::rank_sel::forward_rank_zero,
    crate::traits::rank_sel::forward_select_zero,
    crate::traits::rank_sel::forward_select_hinted,
    crate::traits::rank_sel::forward_select_zero_hinted
];
