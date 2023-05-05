use anyhow::Result;
use crate::traits::*;
use crate::utils::select_in_word;

pub struct SparseZeroIndex<B: SelectZeroHinted, O: VSlice, const QUANTUM_LOG2: usize = 6> {
    bits: B,
    zeros: O,
    _marker: core::marker::PhantomData<[(); QUANTUM_LOG2]>,
}

impl<B: SelectZeroHinted + AsRef<[u64]>, O: VSliceMut, const QUANTUM_LOG2: usize> SparseZeroIndex<B, O, QUANTUM_LOG2>{
    fn build_zeros(&mut self) -> Result<()> {
        let mut number_of_ones = 0;
        let mut next_quantum = 0;
        let mut ones_index = 0;
        for (i, mut word) in self.bits.as_ref().iter().copied().enumerate() {
            word = !word;
            let ones_in_word = word.count_ones() as u64;
            // skip the word if we can
            while number_of_ones + ones_in_word > next_quantum {
                let in_word_index = select_in_word(word, (next_quantum - number_of_ones) as usize);
                let index = (i * 64) as u64 + in_word_index as u64;
                if index >= self.len() as _ {
                    return Ok(());
                }
                self.zeros.set(ones_index, index)?;
                next_quantum += 1 << QUANTUM_LOG2;
                ones_index += 1;
            }

            number_of_ones += ones_in_word;
        }
        Ok(())
    }
}

/// Provide the hint to the underlying structure
impl<B: SelectZeroHinted, O: VSlice, const QUANTUM_LOG2: usize> SelectZero for SparseZeroIndex<B, O, QUANTUM_LOG2> {
    #[inline(always)]
    unsafe fn select_zero_unchecked(&self, rank: usize) -> usize {
        let index = rank >> QUANTUM_LOG2;
        let pos = self.zeros.get_unchecked(index);
        let rank_at_pos = index << QUANTUM_LOG2;

        self.bits.select_zero_unchecked_hinted(
            rank,
            pos as usize,
            rank_at_pos,
        )
    }
}

/// If the underlying implementation has select zero, forward the methods
impl<B: SelectZeroHinted + Select, O: VSlice, const QUANTUM_LOG2: usize> Select for SparseZeroIndex<B, O, QUANTUM_LOG2> {
    #[inline(always)]
    fn select(&self, rank: usize) -> Option<usize> {
        self.bits.select(rank)
    }
    #[inline(always)]
    unsafe fn select_unchecked(&self, rank: usize) -> usize {
        self.bits.select_unchecked(rank)
    }
}

/// If the underlying implementation has select zero, forward the methods
impl<B: SelectZeroHinted + SelectHinted, O: VSlice, const QUANTUM_LOG2: usize> SelectHinted for SparseZeroIndex<B, O, QUANTUM_LOG2> {
    #[inline(always)]
    unsafe fn select_unchecked_hinted(&self, rank: usize, pos: usize, rank_at_pos: usize) -> usize {
        self.bits.select_unchecked_hinted(rank, pos, rank_at_pos)
    }
}

/// Forward the lengths
impl<B: SelectZeroHinted, O: VSlice, const QUANTUM_LOG2: usize> BitLength for SparseZeroIndex<B, O, QUANTUM_LOG2> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.bits.len()
    }
    #[inline(always)]
    fn count(&self) -> usize {
        self.bits.count()
    }
}

impl<B: SelectZeroHinted, const QUANTUM_LOG2: usize> ConvertTo<B> for SparseZeroIndex<B, Vec<u64>, QUANTUM_LOG2> {
    #[inline(always)]
    fn convert_to(self) -> Result<B> {
        Ok(self.bits)
    }
}

impl<B: SelectZeroHinted + AsRef<[u64]>, const QUANTUM_LOG2: usize> ConvertTo<SparseZeroIndex<B, Vec<u64>, QUANTUM_LOG2>> for B {
    #[inline(always)]
    fn convert_to(self) -> Result<SparseZeroIndex<B, Vec<u64>, QUANTUM_LOG2>> {
        let mut res = SparseZeroIndex {
            zeros: vec![0; (self.len() - self.count() + (1 << QUANTUM_LOG2) - 1) >> QUANTUM_LOG2],
            bits: self,
            _marker: core::marker::PhantomData::default(),
        };
        res.build_zeros()?;
        Ok(res)
    }
}

impl<B, O, const QUANTUM_LOG2: usize> AsRef<[u64]> for SparseZeroIndex<B, O, QUANTUM_LOG2> 
where
    B: AsRef<[u64]> + SelectZeroHinted,
    O: VSlice,
{
    fn as_ref(&self) -> &[u64] {
        self.bits.as_ref()
    }
}

impl<B, D, O, const QUANTUM_LOG2: usize> ConvertTo<SparseZeroIndex<B, O, QUANTUM_LOG2>> for SparseZeroIndex<D, O, QUANTUM_LOG2> 
where
    B: SelectZeroHinted + AsRef<[u64]>,
    D: SelectZeroHinted + AsRef<[u64]> + ConvertTo<B>,
    O: VSlice,
{
    #[inline(always)]
    fn convert_to(self) -> Result<SparseZeroIndex<B, O, QUANTUM_LOG2>> {
        Ok(SparseZeroIndex {
            zeros:self.zeros,
            bits: self.bits.convert_to()?,
            _marker: core::marker::PhantomData::default(),
        })
    }
}