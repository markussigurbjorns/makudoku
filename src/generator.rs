use crate::{add_all_sudoku_constraints, types::digit_of_bit, Engine, NN};

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

pub fn generate_full_solution() -> [u8; NN] {
    generate_full_solution_with(|_| {})
}

// TODO: this generates allways the same
// make this more random
pub fn generate_full_solution_with<F>(extra: F) -> [u8; NN]
where
    F: FnOnce(&mut Engine),
{
    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);
    extra(&mut eng);

    eng.search().expect("search failed");
    assert!(eng.solved());

    let mut out = [0u8; NN];
    for i in 0..NN {
        let dom = eng.state.domains[i];
        out[i] = digit_of_bit(dom).unwrap();
    }
    out
}

pub fn generate_puzzle(target_clues: usize) -> String {
    generate_puzzle_with(target_clues, |_| {})
}

// TODO: make seeded generations
pub fn generate_puzzle_with<F>(target_clues: usize, extra: F) -> String
where
    F: Fn(&mut Engine) + Copy,
{
    assert!(target_clues < NN);

    // make a complete solution
    let sol = generate_full_solution_with(extra);
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
        if !has_unique_solution_from_string_with(&puzzle_str, extra) {
            puzzle[pos] = saved;
        }
        let clues_now = puzzle.iter().filter(|c| c.is_some()).count();
        if clues_now <= target_clues {
            break;
        }
    }
    puzzle_vec_to_string(&puzzle)
}

fn _solution_to_string(sol: &[u8; NN]) -> String {
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

fn has_unique_solution_from_string_with<F>(puzzle: &str, extra: F) -> bool
where
    F: Fn(&mut Engine),
{
    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);
    extra(&mut eng);

    if eng.load_givens(puzzle).is_err() {
        return false;
    }

    eng.has_unique_solution()
}

#[cfg(test)]
mod tests {
    use crate::{add_kropki_black, add_kropki_white, add_thermo};

    use super::*;

    #[test]
    fn test() {
        // TODO: MAKE PROPER TESTS
        let white_dots = vec![((0, 0), (0, 1)), ((1, 1), (1, 2))];
        let black_dots = vec![((0, 2), (1, 2))];
        let thermos = vec![
            vec![(0, 0), (1, 0), (2, 0)],
            vec![(4, 4), (4, 5), (4, 6), (4, 7)],
        ];

        let extra = |e: &mut Engine| {
            for &((r1, c1), (r2, c2)) in &white_dots {
                add_kropki_white(e, (r1, c1), (r2, c2));
            }
            for &((r1, c1), (r2, c2)) in &black_dots {
                add_kropki_black(e, (r1, c1), (r2, c2));
            }
            for thermo_cells in &thermos {
                add_thermo(e, thermo_cells);
            }
        };

        let puzzle = generate_puzzle_with(30, extra);
        println!("Kropki+Thermo puzzle:\n{}", puzzle);
    }
}
