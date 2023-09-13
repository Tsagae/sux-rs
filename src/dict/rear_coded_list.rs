/*
 * SPDX-FileCopyrightText: 2023 Inria
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */

use crate::traits::indexed_dict::IndexedDict;
use epserde::traits::*;
use epserde::*;
use num_traits::AsPrimitive;

#[derive(Debug, Clone, Default, Epserde)]
/// Statistics of the encoded data
pub struct Stats {
    /// Maximum block size in bytes
    pub max_block_bytes: usize,
    /// The total sum of the block size in bytes
    pub sum_block_bytes: usize,

    /// Maximum shared prefix in bytes
    pub max_lcp: usize,
    /// The total sum of the shared prefix in bytes
    pub sum_lcp: usize,

    /// maximum string length in bytes
    pub max_str_len: usize,
    /// the total sum of the strings length in bytes
    pub sum_str_len: usize,

    /// The number of bytes used to store the rear lengths in data
    pub code_bytes: usize,
    /// The number of bytes used to store the suffixes in data
    pub suffixes_bytes: usize,

    /// The bytes wasted writing without compression the first string in block
    pub redundancy: isize,
}

#[derive(Debug, Epserde)]
/// Rear coded list, it takes a list of strings and encode them in a way that
/// the common prefix between strings is encoded only once.
///
/// The encoding is done in blocks of k strings, the first string is encoded
/// without compression, the other strings are encoded with the common prefix
/// removed.
///
/// The encoding is done in a way that the encoded strings are \0 terminated
/// and the pointers to the start of the strings are stored in a separate
/// structure `Ptr`. This structure could be either arrays, possibly memory-mapped,
/// of different sized of ptrs, or Elias-Fano, or any other structure that can
/// store monotone increasing integers.
pub struct RearCodedList<Ptr: AsPrimitive<usize> + ZeroCopy = usize>
where
    usize: AsPrimitive<Ptr>,
{
    /// The encoded strings \0 terminated
    data: Vec<u8>,
    /// The pointer to in which byte the k-th string start
    pointers: Vec<Ptr>,
    /// The number of strings in a block, this regulates the compression vs
    /// decompression speed tradeoff
    k: usize,
    /// Statistics of the encoded data
    pub stats: Stats,
    /// Number of encoded strings
    len: usize,
    /// Cache of the last encoded string for incremental encoding
    last_str: Vec<u8>,
}

/// Copy a string until the first \0 from `data` to `result` and return the
/// remaining data
#[inline(always)]
fn strcpy<'a>(mut data: &'a [u8], result: &mut Vec<u8>) -> &'a [u8] {
    loop {
        let c = data[0];
        data = &data[1..];
        if c == 0 {
            break;
        }
        result.push(c);
    }
    data
}

#[inline(always)]
/// strcmp but string is a rust string and data is a \0 terminated string
fn strcmp(string: &[u8], data: &[u8]) -> core::cmp::Ordering {
    for (i, c) in string.iter().enumerate() {
        match data[i].cmp(c) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
    }
    // string has an implicit final \0
    data[string.len()].cmp(&0)
}

#[inline(always)]
/// strcmp but both string are rust strings
fn strcmp_rust(string: &[u8], other: &[u8]) -> core::cmp::Ordering {
    for (i, c) in string.iter().enumerate() {
        match other.get(i).unwrap_or(&0).cmp(c) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
    }
    // string has an implicit final \0
    other.len().cmp(&string.len())
}

impl<Ptr: AsPrimitive<usize> + ZeroCopy> RearCodedList<Ptr>
where
    usize: AsPrimitive<Ptr>,
{
    /// If true, compute the redundancy of the encoding, this is useful to
    /// understand how much the encoding is compressing the data.
    /// But slows down the construction as we need to compute the LCP even on
    /// strings that are not compressed.
    const COMPUTE_REDUNDANCY: bool = true;

    /// Create a new empty RearCodedList where the block size is `k`.
    /// This means that the first string every `k` is encoded without compression,
    /// the other strings are encoded with the common prefix removed.
    pub fn new(k: usize) -> Self {
        Self {
            data: Vec::with_capacity(1 << 20),
            last_str: Vec::with_capacity(1024),
            pointers: Vec::new(),
            len: 0,
            k,
            stats: Default::default(),
        }
    }

    /// Re-allocate the data to remove wasted capacity in the structure
    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
        self.pointers.shrink_to_fit();
        self.last_str.shrink_to_fit();
    }

    #[inline]
    /// Append a string to the end of the list
    pub fn push<S: AsRef<str>>(&mut self, string: S) {
        let string = string.as_ref();
        // update stats
        self.stats.max_str_len = self.stats.max_str_len.max(string.len());
        self.stats.sum_str_len += string.len();

        // at every multiple of k we just encode the string as is
        let to_encode = if self.len % self.k == 0 {
            // compute the size in bytes of the previous block
            let last_ptr = self.pointers.last().copied().unwrap_or(0.as_());
            let block_bytes = self.data.len() - last_ptr.as_();
            // update stats
            self.stats.max_block_bytes = self.stats.max_block_bytes.max(block_bytes);
            self.stats.sum_block_bytes += block_bytes;
            // save a pointer to the start of the string
            self.pointers.push(self.data.len().as_());

            // compute the redundancy
            if Self::COMPUTE_REDUNDANCY {
                let lcp = longest_common_prefix(&self.last_str, string.as_bytes());
                let rear_length = self.last_str.len() - lcp;
                self.stats.redundancy += lcp as isize;
                self.stats.redundancy -= encode_int_len(rear_length) as isize;
            }

            // just encode the whole string
            string.as_bytes()
        } else {
            // just write the difference between the last string and the current one
            // encode only the delta
            let lcp = longest_common_prefix(&self.last_str, string.as_bytes());
            // update the stats
            self.stats.max_lcp = self.stats.max_lcp.max(lcp);
            self.stats.sum_lcp += lcp;
            // encode the len of the bytes in data
            let rear_length = self.last_str.len() - lcp;
            let prev_len = self.data.len();
            encode_int(rear_length, &mut self.data);
            // update stats
            self.stats.code_bytes += self.data.len() - prev_len;
            // return the delta suffix
            &string.as_bytes()[lcp..]
        };
        // Write the data to the buffer
        self.data.extend_from_slice(to_encode);
        // push the \0 terminator
        self.data.push(0);
        self.stats.suffixes_bytes += to_encode.len() + 1;

        // put the string as last_str for the next iteration
        self.last_str.clear();
        self.last_str.extend_from_slice(string.as_bytes());
        self.len += 1;
    }

    #[inline]
    /// Append all the strings from an iterator to the end of the list
    pub fn extend<S: AsRef<str>, I: Iterator<Item = S>>(&mut self, iter: I) {
        for string in iter {
            self.push(string);
        }
    }

    /// Write the index-th string to `result` as bytes. This is done to avoid
    /// allocating a new string for every query and skipping the utf-8 validity
    /// check.
    #[inline(always)]
    pub fn get_inplace(&self, index: usize, result: &mut Vec<u8>) {
        result.clear();
        let block = index / self.k;
        let offset = index % self.k;

        let start = self.pointers[block];
        let data = &self.data[start.as_()..];

        // decode the first string in the block
        let mut data = strcpy(data, result);

        for _ in 0..offset {
            // get how much data to throw away
            let (len, tmp) = decode_int(data);
            // throw away the data
            result.resize(result.len() - len, 0);
            // copy the new suffix
            let tmp = strcpy(tmp, result);
            data = tmp;
        }
    }

    /// Return whether the string is contained in the array.
    /// This can be used only if the strings inserted were sorted.
    pub fn contains(&self, string: &str) -> bool {
        let string = string.as_bytes();
        // first to a binary search on the blocks to find the block
        let block_idx = self
            .pointers
            .binary_search_by(|block_ptr| strcmp(string, &self.data[block_ptr.as_()..]));

        if block_idx.is_ok() {
            return true;
        }

        let mut block_idx = block_idx.unwrap_err();
        if block_idx == 0 || block_idx > self.pointers.len() {
            // the string is before the first block
            return false;
        }
        block_idx -= 1;
        // finish by a linear search on the block
        let mut result = Vec::with_capacity(self.stats.max_str_len);
        let start = self.pointers[block_idx];
        let data = &self.data[start.as_()..];

        // decode the first string in the block
        let mut data = strcpy(data, &mut result);
        let in_block = (self.k - 1).min(self.len - block_idx * self.k - 1);
        for _ in 0..in_block {
            // get how much data to throw away
            let (len, tmp) = decode_int(data);
            let lcp = result.len() - len;
            // throw away the data
            result.resize(lcp, 0);
            // copy the new suffix
            let tmp = strcpy(tmp, &mut result);
            data = tmp;

            // TODO!: this can be optimized to avoid the copy
            match strcmp_rust(string, &result) {
                core::cmp::Ordering::Less => {}
                core::cmp::Ordering::Equal => return true,
                core::cmp::Ordering::Greater => return false,
            }
        }
        false
    }

    /// Return a sequential iterator over the strings
    pub fn iter(&self) -> RCAIter<'_, Ptr> {
        RCAIter {
            rca: self,
            index: 0,
            data: &self.data,
            buffer: Vec::with_capacity(self.stats.max_str_len),
        }
    }

    // create a sequential iterator from a given index
    pub fn iter_from(&self, index: usize) -> RCAIter<'_, Ptr> {
        let block = index / self.k;
        let offset = index % self.k;

        let start = self.pointers[block];
        let mut res = RCAIter {
            rca: self,
            index,
            data: &self.data[start.as_()..],
            buffer: Vec::with_capacity(self.stats.max_str_len),
        };
        for _ in 0..offset {
            res.next();
        }
        res
    }

    /// Print in an human readable format the statistics of the RCL
    pub fn print_stats(&self) {
        println!(
            "{:>20}: {:>10}",
            "max_block_bytes", self.stats.max_block_bytes
        );
        println!(
            "{:>20}: {:>10.3}",
            "avg_block_bytes",
            self.stats.sum_block_bytes as f64 / self.len() as f64
        );

        println!("{:>20}: {:>10}", "max_lcp", self.stats.max_lcp);
        println!(
            "{:>20}: {:>10.3}",
            "avg_lcp",
            self.stats.sum_lcp as f64 / self.len() as f64
        );

        println!("{:>20}: {:>10}", "max_str_len", self.stats.max_str_len);
        println!(
            "{:>20}: {:>10.3}",
            "avg_str_len",
            self.stats.sum_str_len as f64 / self.len() as f64
        );

        let ptr_size: usize = self.pointers.len() * core::mem::size_of::<Ptr>();

        fn human(key: &str, x: usize) {
            const UOM: &[&str] = &["B", "KB", "MB", "GB", "TB"];
            let mut y = x as f64;
            let mut uom_idx = 0;
            while y > 1000.0 {
                uom_idx += 1;
                y /= 1000.0;
            }
            println!("{:>20}:{:>10.3}{}{:>20} ", key, y, UOM[uom_idx], x);
        }

        let total_size = ptr_size + self.data.len() + core::mem::size_of::<Self>();
        human("data_bytes", self.data.len());
        human("codes_bytes", self.stats.code_bytes);
        human("suffixes_bytes", self.stats.suffixes_bytes);
        human("ptrs_bytes", ptr_size);
        human("uncompressed_size", self.stats.sum_str_len);
        human("total_size", total_size);

        if Self::COMPUTE_REDUNDANCY {
            human(
                "optimal_size",
                (self.data.len() as isize - self.stats.redundancy) as usize,
            );
            human("redundancy", self.stats.redundancy as usize);
            let overhead = self.stats.redundancy + ptr_size as isize;
            println!(
                "overhead_ratio: {:>10}",
                overhead as f64 / (overhead + self.data.len() as isize) as f64
            );
            println!(
                "no_overhead_compression_ratio: {:.3}",
                (self.data.len() as isize - self.stats.redundancy) as f64
                    / self.stats.sum_str_len as f64
            );
        }

        println!(
            "compression_ratio: {:.3}",
            (ptr_size + self.data.len()) as f64 / self.stats.sum_str_len as f64
        );
    }
}

impl<Ptr: AsPrimitive<usize> + ZeroCopy> IndexedDict for RearCodedList<Ptr>
where
    usize: AsPrimitive<Ptr>,
{
    type Value = String;

    unsafe fn get_unchecked(&self, index: usize) -> Self::Value {
        let mut result = Vec::with_capacity(self.stats.max_str_len);
        self.get_inplace(index, &mut result);
        String::from_utf8(result).unwrap()
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.len
    }
}

/// Sequential iterator over the strings
pub struct RCAIter<'a, Ptr: AsPrimitive<usize> + ZeroCopy>
where
    usize: AsPrimitive<Ptr>,
{
    rca: &'a RearCodedList<Ptr>,
    buffer: Vec<u8>,
    data: &'a [u8],
    index: usize,
}

impl<'a, Ptr: AsPrimitive<usize> + ZeroCopy> RCAIter<'a, Ptr>
where
    usize: AsPrimitive<Ptr>,
{
    pub fn new(rca: &'a RearCodedList<Ptr>) -> Self {
        Self {
            rca,
            buffer: Vec::with_capacity(rca.stats.max_str_len),
            data: &rca.data,
            index: 0,
        }
    }
}

impl<'a, Ptr: AsPrimitive<usize> + ZeroCopy> Iterator for RCAIter<'a, Ptr>
where
    usize: AsPrimitive<Ptr>,
{
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.rca.len() {
            return None;
        }

        if self.index % self.rca.k == 0 {
            // just copy the data
            self.buffer.clear();
            self.data = strcpy(self.data, &mut self.buffer);
        } else {
            let (len, tmp) = decode_int(self.data);
            self.buffer.resize(self.buffer.len() - len, 0);
            self.data = strcpy(tmp, &mut self.buffer);
        }
        self.index += 1;

        Some(String::from_utf8(self.buffer.clone()).unwrap())
    }
}

impl<'a, Ptr: AsPrimitive<usize> + ZeroCopy> ExactSizeIterator for RCAIter<'a, Ptr>
where
    usize: AsPrimitive<Ptr>,
{
    fn len(&self) -> usize {
        self.rca.len() - self.index
    }
}

#[inline(always)]
/// Compute the longest common prefix between two strings as bytes
fn longest_common_prefix(a: &[u8], b: &[u8]) -> usize {
    // ofc the lcp is at most the len of the minimum string
    let min_len = a.len().min(b.len());
    // normal lcp computation
    let mut i = 0;
    while i < min_len && a[i] == b[i] {
        i += 1;
    }
    // TODO!: try to optimize with vpcmpeqb pextrb and leading count ones
    i
}

#[cfg(test)]
#[cfg_attr(test, test)]
fn test_longest_common_prefix() {
    let str1 = b"absolutely";
    let str2 = b"absorption";
    assert_eq!(longest_common_prefix(str1, str2), 4);
    assert_eq!(longest_common_prefix(str1, str1), str1.len());
    assert_eq!(longest_common_prefix(str2, str2), str2.len());
}

/// Compute the length in bytes of value encoded as VByte
#[inline(always)]
fn encode_int_len(mut value: usize) -> usize {
    let mut len = 1;
    let mut max = 1 << 7;
    while value >= max {
        len += 1;
        value -= max;
        max <<= 7;
    }
    len
}

const UPPER_BOUND_1: usize = 128;
const UPPER_BOUND_2: usize = 128_usize.pow(2) + UPPER_BOUND_1;
const UPPER_BOUND_3: usize = 128_usize.pow(3) + UPPER_BOUND_2;
const UPPER_BOUND_4: usize = 128_usize.pow(4) + UPPER_BOUND_3;
const UPPER_BOUND_5: usize = 128_usize.pow(5) + UPPER_BOUND_4;
const UPPER_BOUND_6: usize = 128_usize.pow(6) + UPPER_BOUND_5;
const UPPER_BOUND_7: usize = 128_usize.pow(7) + UPPER_BOUND_6;
const UPPER_BOUND_8: usize = 128_usize.pow(8) + UPPER_BOUND_7;

/// VByte encode an integer
#[inline(always)]
fn encode_int(mut value: usize, data: &mut Vec<u8>) {
    if value < UPPER_BOUND_1 {
        data.push(value as u8);
        return;
    }
    if value < UPPER_BOUND_2 {
        value -= UPPER_BOUND_1;
        debug_assert!((value >> 8) < (1 << 6));
        data.push(0x80 | (value >> 8) as u8);
        data.push(value as u8);
        return;
    }
    if value < UPPER_BOUND_3 {
        value -= UPPER_BOUND_2;
        debug_assert!((value >> 16) < (1 << 5));
        data.push(0xC0 | (value >> 16) as u8);
        data.push((value >> 8) as u8);
        data.push(value as u8);
        return;
    }
    if value < UPPER_BOUND_4 {
        value -= UPPER_BOUND_3;
        debug_assert!((value >> 24) < (1 << 4));
        data.push(0xE0 | (value >> 24) as u8);
        data.push((value >> 16) as u8);
        data.push((value >> 8) as u8);
        data.push(value as u8);
        return;
    }
    if value < UPPER_BOUND_5 {
        value -= UPPER_BOUND_4;
        debug_assert!((value >> 32) < (1 << 3));
        data.push(0xF0 | (value >> 32) as u8);
        data.push((value >> 24) as u8);
        data.push((value >> 16) as u8);
        data.push((value >> 8) as u8);
        data.push(value as u8);
        return;
    }
    if value < UPPER_BOUND_6 {
        value -= UPPER_BOUND_5;
        debug_assert!((value >> 40) < (1 << 2));
        data.push(0xF8 | (value >> 40) as u8);
        data.push((value >> 32) as u8);
        data.push((value >> 24) as u8);
        data.push((value >> 16) as u8);
        data.push((value >> 8) as u8);
        data.push(value as u8);
        return;
    }
    if value < UPPER_BOUND_7 {
        value -= UPPER_BOUND_6;
        debug_assert!((value >> 48) < (1 << 1));
        data.push(0xFC | (value >> 48) as u8);
        data.push((value >> 40) as u8);
        data.push((value >> 32) as u8);
        data.push((value >> 24) as u8);
        data.push((value >> 16) as u8);
        data.push((value >> 8) as u8);
        data.push(value as u8);
        return;
    }
    if value < UPPER_BOUND_8 {
        value -= UPPER_BOUND_7;
        data.push(0xFE);
        data.push((value >> 48) as u8);
        data.push((value >> 40) as u8);
        data.push((value >> 32) as u8);
        data.push((value >> 24) as u8);
        data.push((value >> 16) as u8);
        data.push((value >> 8) as u8);
        data.push(value as u8);
        return;
    }

    data.push(0xFF);
    data.push((value >> 56) as u8);
    data.push((value >> 48) as u8);
    data.push((value >> 40) as u8);
    data.push((value >> 32) as u8);
    data.push((value >> 24) as u8);
    data.push((value >> 16) as u8);
    data.push((value >> 8) as u8);
    data.push(value as u8);
}

#[inline(always)]
fn decode_int(data: &[u8]) -> (usize, &[u8]) {
    let x = data[0];
    if x < 0x80 {
        return (x as usize, &data[1..]);
    }
    if x < 0xC0 {
        let x = (((x & !0xC0) as usize) << 8 | data[1] as usize) + UPPER_BOUND_1;
        return (x, &data[2..]);
    }
    if x < 0xE0 {
        let x = (((x & !0xE0) as usize) << 16 | (data[1] as usize) << 8 | data[2] as usize)
            + UPPER_BOUND_2;
        return (x, &data[3..]);
    }
    if x < 0xF0 {
        let x = (((x & !0xF0) as usize) << 24
            | (data[1] as usize) << 16
            | (data[2] as usize) << 8
            | data[3] as usize)
            + UPPER_BOUND_3;
        return (x, &data[4..]);
    }
    if x < 0xF8 {
        let x = (((x & !0xF8) as usize) << 32
            | (data[1] as usize) << 24
            | (data[2] as usize) << 16
            | (data[3] as usize) << 8
            | data[4] as usize)
            + UPPER_BOUND_4;
        return (x, &data[5..]);
    }
    if x < 0xFC {
        let x = (((x & !0xFC) as usize) << 40
            | (data[1] as usize) << 32
            | (data[2] as usize) << 24
            | (data[3] as usize) << 16
            | (data[4] as usize) << 8
            | data[5] as usize)
            + UPPER_BOUND_5;
        return (x, &data[6..]);
    }
    if x < 0xFE {
        let x = (((x & !0xFE) as usize) << 48
            | (data[1] as usize) << 40
            | (data[2] as usize) << 32
            | (data[3] as usize) << 24
            | (data[4] as usize) << 16
            | (data[5] as usize) << 8
            | data[6] as usize)
            + UPPER_BOUND_6;
        return (x, &data[7..]);
    }
    if x < 0xFF {
        let x = ((data[1] as usize) << 48
            | (data[2] as usize) << 40
            | (data[3] as usize) << 32
            | (data[4] as usize) << 24
            | (data[5] as usize) << 16
            | (data[6] as usize) << 8
            | data[7] as usize)
            + UPPER_BOUND_7;
        return (x, &data[8..]);
    }

    let x = (data[1] as usize) << 56
        | (data[2] as usize) << 48
        | (data[3] as usize) << 40
        | (data[4] as usize) << 32
        | (data[5] as usize) << 24
        | (data[6] as usize) << 16
        | (data[7] as usize) << 8
        | data[8] as usize;
    (x, &data[9..])
}

#[cfg(test)]
#[cfg_attr(test, test)]
fn test_encode_decode_int() {
    const MAX: usize = 1 << 20;
    const MIN: usize = 0;
    let mut buffer = Vec::with_capacity(128);

    for i in MIN..MAX {
        encode_int(i, &mut buffer);
    }

    let mut data = &buffer[..];
    for i in MIN..MAX {
        let (j, tmp) = decode_int(data);
        assert_eq!(data.len() - tmp.len(), encode_int_len(i));
        data = tmp;
        assert_eq!(i, j);
    }
}