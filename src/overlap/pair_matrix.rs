use std::ops::{Index, IndexMut};
use crate::overlap::tracker::OTEntry;

// Triangular matrix of pair-wise overlaps and weights
#[derive(Debug, Clone)]
pub struct PairMatrix {
    pub size: usize,
    pub data: Vec<OTEntry>,
}

impl PairMatrix {
    pub fn new(size: usize) -> Self {
        let len = size * (size + 1) / 2;
        Self {
            size,
            data: vec![OTEntry::default(); len],
        }
    }

    pub fn clear_overlaps_for(&mut self, i: usize) {
        for j in 0..self.size {
            self[(j, i)].overlap = 0.0;
            self[(i, j)].overlap = 0.0;
        }
    }

    pub fn row_iter(&self, row: usize) -> impl Iterator<Item=&OTEntry> {
        (0..self.size).map(move |col| &self[(row, col)])
    }
}

impl Index<(usize, usize)> for PairMatrix {
    type Output = OTEntry;

    fn index(&self, (row, col): (usize, usize)) -> &Self::Output {
        &self.data[calc_idx(row, col, self.size)]
    }
}

impl IndexMut<(usize, usize)> for PairMatrix {
    fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut Self::Output {
        &mut self.data[calc_idx(row, col, self.size)]
    }
}


fn calc_idx(i: usize, j: usize, n: usize) -> usize {
    //https://stackoverflow.com/questions/3187957/how-to-store-a-symmetric-matrix
    /* Example:
        0 1 2 3
          4 5 6
            7 8
              9
    */

    debug_assert!(i < n && j < n);
    if (i <= j) {
        i * n - (i - 1) * i / 2 + j - i
    }
    else {
        j * n - (j - 1) * j / 2 + i - j
    }
}