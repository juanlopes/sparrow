use crate::overlap::tracker::OTEntry;
use std::ops::{Index, IndexMut};

// triangular matrix of pair-wise overlaps and weights
// supporting data structure for the OverlapTracker
#[derive(Debug, Clone)]
pub struct OTPairMatrix {
    pub size: usize,
    pub data: Vec<OTEntry>,
}

impl OTPairMatrix {
    pub fn new(size: usize) -> Self {
        let len = size * (size + 1) / 2;
        Self {
            size,
            data: vec![OTEntry { weight: 1.0, overlap: 0.0 }; len],
        }
    }
}

impl Index<(usize, usize)> for OTPairMatrix {
    type Output = OTEntry;

    fn index(&self, (row, col): (usize, usize)) -> &Self::Output {
        &self.data[calc_idx(row, col, self.size)]
    }
}

impl IndexMut<(usize, usize)> for OTPairMatrix {
    fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut Self::Output {
        &mut self.data[calc_idx(row, col, self.size)]
    }
}

fn calc_idx(row: usize, col: usize, size: usize) -> usize {
    /* Example:
        0 1 2 3
          4 5 6
            7 8
              9
    */
    debug_assert!(row < size && col < size);
    if row <= col {
        (row * size) + col - ((row * (row + 1)) / 2)
    } else {
        (col * size) + row - ((col * (col + 1)) / 2)
    }
}
