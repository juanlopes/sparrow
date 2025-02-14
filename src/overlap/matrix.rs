use std::ops::{Index, IndexMut};
use float_cmp::assert_approx_eq;
use jagua_rs::fsize;
use jagua_rs::util::fpa::FPA;

#[derive(Debug, Clone)]
pub struct Matrix<T: Clone> {
    pub size: usize,
    pub default: T,
    pub data: Box<[T]>,
}

impl<T: Clone> Matrix<T> {
    pub fn new(size: usize, default: T) -> Self {
        let len = size * size;
        Self {
            size,
            default: default.clone(),
            data: vec![default.clone(); len].into_boxed_slice(),
        }
    }

    pub fn reset_row_and_col(&mut self, idx: usize){
        for i in 0..self.size {
            self[(idx, i)] = self.default.clone();
            self[(i, idx)] = self.default.clone();
        }
    }

    pub fn row(&self, row: usize) -> &[T] {
        &self.data[row * self.size..(row + 1) * self.size]
    }
}

impl<T: Clone> Index<(usize, usize)> for Matrix<T> {
    type Output = T;

    fn index(&self, (row, col): (usize, usize)) -> &Self::Output {
        &self.data[row * self.size + col]
    }
}

impl<T: Clone> IndexMut<(usize, usize)> for Matrix<T> {
    fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut Self::Output {
        &mut self.data[row * self.size + col]
    }
}

pub fn assert_matrix_symmetrical(m: &Matrix<fsize>) -> bool {
    for i in 0..m.size {
        for j in 0..i {
            let v1 = m[(i, j)];
            let v2 = m[(j,i)];
            assert_approx_eq!(fsize, v1, v2, epsilon = FPA::tolerance())
        }
    }
    true
}