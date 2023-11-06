/*
 *
 * SPDX-FileCopyrightText: 2023 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-or-later
 */

/*!

Utility wrappers for files.

*/

use flate2::read::GzDecoder;
use io::{BufRead, BufReader};
use lender::*;
use std::{io, path::Path};
use zstd::stream::read::Decoder;

/**

A structure lending the lines coming from a [`BufRead`] as `&str`.

The lines are read into a reusable internal string buffer that
grows as needed.

For convenience, we implement [`From`] from [`BufRead`].

*/
pub struct LineLender<B> {
    buf: B,
    line: String,
}

impl<B> LineLender<B> {
    pub fn new(buf: B) -> Self {
        LineLender {
            buf,
            line: String::with_capacity(128),
        }
    }
}

impl<B> From<B> for LineLender<B> {
    fn from(buf: B) -> Self {
        LineLender::new(buf)
    }
}

impl<'lend, B: BufRead> Lending<'lend> for LineLender<B> {
    type Lend = io::Result<&'lend str>;
}

impl<B: BufRead> Lender for LineLender<B> {
    fn next(&mut self) -> Option<Lend<'_, Self>> {
        self.line.clear();
        match self.buf.read_line(&mut self.line) {
            Err(e) => Some(Err(e)),
            Ok(0) => None,
            Ok(_) => {
                if self.line.ends_with('\n') {
                    self.line.pop();
                    if self.line.ends_with('\r') {
                        self.line.pop();
                    }
                }
                Some(Ok(&self.line))
            }
        }
    }
}

/// Adapter to iterate over the lines of a file.
#[derive(Clone)]
pub struct FilenameIntoLender<P: AsRef<Path>>(pub P);

impl<P: AsRef<Path>> TryFrom<FilenameIntoLender<P>> for LineLender<BufReader<std::fs::File>> {
    type Error = io::Error;
    fn try_from(path: FilenameIntoLender<P>) -> io::Result<LineLender<BufReader<std::fs::File>>> {
        Ok(BufReader::new(std::fs::File::open(path.0)?).into())
    }
}

/// Adapter to iterate over the lines of a file compressed with Zstandard.
#[derive(Clone)]
pub struct FilenameZstdIntoLender<P: AsRef<Path>>(pub P);

impl<P: AsRef<Path>> IntoLender for FilenameZstdIntoLender<P> {
    type Lender = LineLender<BufReader<Decoder<'static, BufReader<std::fs::File>>>>;

    fn into_lender(self) -> Self::Lender {
        LineLender {
            buf: BufReader::new(Decoder::new(std::fs::File::open(self.0).unwrap()).unwrap()),
            line: String::new(),
        }
    }
}

impl<P: AsRef<Path>> From<P> for FilenameZstdIntoLender<P> {
    fn from(path: P) -> Self {
        FilenameZstdIntoLender(path)
    }
}

/// Adapter to iterate over the lines of a file compressed with Gzip.
#[derive(Clone)]
pub struct FilenameGzipIntoLender<P: AsRef<Path>>(pub P);

impl<P: AsRef<Path>> IntoLender for FilenameGzipIntoLender<P> {
    type Lender = LineLender<BufReader<GzDecoder<std::fs::File>>>;

    fn into_lender(self) -> Self::Lender {
        LineLender {
            buf: BufReader::new(GzDecoder::new(std::fs::File::open(self.0).unwrap())),
            line: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct OkIterator<I>(pub I);

impl<I: Iterator> Iterator for OkIterator<I> {
    type Item = io::Result<I::Item>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|x| Ok(x))
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct IntoOkIterator<I>(pub I);

impl<I: IntoIterator> IntoIterator for IntoOkIterator<I> {
    type Item = io::Result<I::Item>;
    type IntoIter = OkIterator<I::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        OkIterator(self.0.into_iter())
    }
}
#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct OkLender<I>(pub I);

impl<'lend, I: Lender> Lending<'lend> for OkLender<I> {
    type Lend = io::Result<Lend<'lend, I>>;
}

impl<I: Lender> Lender for OkLender<I> {
    fn next(&mut self) -> Option<Lend<'_, Self>> {
        self.0.next().map(|x| Ok(x))
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct IntoOkLender<I>(pub I);

impl<I: IntoLender> IntoLender for IntoOkLender<I> {
    type Lender = OkLender<I::Lender>;

    fn into_lender(self) -> Self::Lender {
        OkLender(self.0.into_lender())
    }
}

impl<I: IntoLender> From<I> for IntoOkLender<I> {
    fn from(into_iter: I) -> Self {
        IntoOkLender(into_iter)
    }
}

#[derive(Clone, Debug)]
pub struct RefLender<I: Iterator> {
    iter: I,
    item: Option<I::Item>,
}

impl<'lend, I: Iterator> Lending<'lend> for RefLender<I> {
    type Lend = &'lend I::Item;
}

impl<I: Iterator> Lender for RefLender<I> {
    fn next(&mut self) -> Option<Lend<'_, Self>> {
        self.item = self.iter.next();
        self.item.as_ref()
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct IntoRefLender<I: IntoIterator>(pub I);

impl<I: IntoIterator> IntoLender for IntoRefLender<I> {
    type Lender = RefLender<I::IntoIter>;

    fn into_lender(self) -> <Self as IntoLender>::Lender {
        RefLender {
            iter: self.0.into_iter(),
            item: None,
        }
    }
}
