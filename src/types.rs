pub const N: usize = 9;
pub const NN: usize = N * N; //81

pub type CellIx = u8; //0..80
pub type Domain = u16; //bits 1..=9 used

pub const DIGITS_MASK: Domain = 0b_11_1111_1110;
pub const EVEN_MASK: Domain = (1 << 2) | (1 << 4) | (1 << 6) | (1 << 8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Solve {
    Solved,
    Progress,
    Stalled,
}

#[derive(Debug)]
pub struct Contradiction;

#[inline]
pub fn row_of(i: CellIx) -> usize {
    (i as usize) / N
}
#[inline]
pub fn col_of(i: CellIx) -> usize {
    (i as usize) % N
}
#[inline]
pub fn box_of(i: CellIx) -> usize {
    (row_of(i) / 3) * 3 + (col_of(i) / 3)
}
#[inline]
pub fn idx(r: usize, c: usize) -> CellIx {
    (r * N + c) as CellIx
}

#[inline]
pub fn bit_of_digit(d: u8) -> Domain {
    1u16 << d
}

#[inline]
pub fn _digit_of_bit(bit: Domain) -> Option<u8> {
    if bit == 0 || !bit.is_power_of_two() {
        None
    } else {
        Some(bit.trailing_zeros() as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_of_digit() {
        let res = bit_of_digit(4);
        assert_eq!(res, 0b1_0000);
    }
}
