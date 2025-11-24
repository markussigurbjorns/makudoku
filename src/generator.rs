use crate::{Engine, NN, add_all_sudoku_constraints, types::digit_of_bit};

use std::{
    time::{SystemTime, UNIX_EPOCH},
    usize,
};

struct SimpleRng(u64);

impl SimpleRng {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(nanos as u64)
    }

    fn next_u32(&mut self) -> u32 {
        // LCG parameters from Numerical Recipes
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 32) as u32
    }

    fn gen_range(&mut self, range: std::ops::Range<usize>) -> usize {
        let len = range.end - range.start;
        assert!(len > 0);
        let v = self.next_u32() as usize % len;
        range.start + v
    }
}

fn shuffle<T>(rng: &mut SimpleRng, slice: &mut [T]) {
    let len = slice.len();
    if len <= 1 {
        return;
    }

    for i in (1..len).rev() {
        let j = rng.gen_range(0..i + 1); // random in [0, i]
        slice.swap(i, j);
    }
}

// TODO: this generates allways the same
// make this more random
pub fn generate_full_solution() -> [u8; NN] {
    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);
    eng.search().expect("search failed");
    assert!(eng.solved());

    let mut out = [0u8; NN];
    for i in 0..NN {
        let dom = eng.state.domains[i];
        out[i] = digit_of_bit(dom).unwrap();
    }
    out
}

// TODO: make seeded generations
pub fn generate_puzzle(target_clues: usize) -> String {
    assert!(target_clues < NN);

    // make a complete solution
    let sol = generate_full_solution();
    let mut puzzle: Vec<Option<u8>> = sol.iter().copied().map(Some).collect();

    // random order of position try to remove
    let mut positions: Vec<usize> = (0..NN).collect();
    let mut rng = SimpleRng::new();
    shuffle(&mut rng, &mut positions);

    // try to remove clues while preserving uniqueness
    for pos in positions {
        let saved = puzzle[pos];
        puzzle[pos] = None;

        let puzzle_str = puzzle_vec_to_string(&puzzle);
        if !has_unique_solution_from_string(&puzzle_str) {
            puzzle[pos] = saved;
        }
        let clues_now = puzzle.iter().filter(|c| c.is_some()).count();
        if clues_now <= target_clues {
            break;
        }
    }
    puzzle_vec_to_string(&puzzle)
}

fn solution_to_string(sol: &[u8; NN]) -> String {
    let mut s = String::with_capacity(NN);
    for &d in sol.iter() {
        s.push((b'0' + d) as char);
    }
    s
}

fn puzzle_vec_to_string(puzzle: &[Option<u8>]) -> String {
    let mut s = String::with_capacity(NN);
    for cell in puzzle.iter() {
        match cell {
            Some(d) => s.push((b'0' + *d) as char),
            None => s.push('.'),
        }
    }
    s
}

fn has_unique_solution_from_string(puzzle: &str) -> bool {
    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);

    if eng.load_givens(puzzle).is_err() {
        return false;
    }

    eng.has_unique_solution()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let puzzle = generate_puzzle(20);
        println!("{}", puzzle);
    }
}
