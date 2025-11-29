use crate::{Engine, NN, add_all_sudoku_constraints, types::digit_of_bit};

use std::{
    time::{SystemTime, UNIX_EPOCH},
    usize,
};

#[derive(Clone)]
pub struct SimpleRng(u64);

impl SimpleRng {
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(nanos as u64)
    }

    pub fn from_seed(seed: u64) -> Self {
        Self(seed)
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

pub fn generate_full_solution(rng: SimpleRng) -> [u8; NN] {
    generate_full_solution_with(rng, |_| {})
}

pub fn generate_full_solution_with<F>(mut rng: SimpleRng, extra: F) -> [u8; NN]
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

    if eng.constraints.is_empty() {
        let mut digits = [1u8, 2, 3, 4, 5, 6, 7, 8, 9];
        shuffle(&mut rng, &mut digits);

        let mut perm = [0u8; 10];
        for (i, d) in digits.iter().enumerate() {
            perm[i + 1] = *d;
        }
        for cell in out.iter_mut() {
            *cell = perm[*cell as usize];
        }
    }
    out
}

pub fn generate_puzzle(target_clues: usize, rng: SimpleRng) -> String {
    generate_puzzle_with(target_clues, rng, |_| {})
}

pub fn generate_puzzle_with<F>(target_clues: usize, mut rng: SimpleRng, extra: F) -> String
where
    F: Fn(&mut Engine) + Copy,
{
    assert!(target_clues < NN);

    // make a complete solution
    let sol = generate_full_solution_with(rng.clone(), extra);
    let mut puzzle: Vec<Option<u8>> = sol.iter().copied().map(Some).collect();

    // random order of position try to remove
    let mut positions: Vec<usize> = (0..NN).collect();
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
    use crate::{
        Engine, add_all_sudoku_constraints, add_kropki_black, add_kropki_white, add_thermo,
    };

    use super::*;

    fn clue_count(puzzle: &str) -> usize {
        puzzle.chars().filter(|ch| matches!(ch, '1'..='9')).count()
    }

    #[test]
    fn test_generate_puzzle_with_seed() {
        let rng = SimpleRng::from_seed(12134);
        let puzzle = generate_puzzle_with(80, rng, |_| {});
        assert_eq!(clue_count(&puzzle), 80);
        let mut eng = Engine::new();
        add_all_sudoku_constraints(&mut eng);
        eng.load_givens(&puzzle).unwrap();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
        assert!(eng.has_unique_solution());
    }

    #[test]
    fn generate_full_solution_with_seed_is_valid_grid() {
        let rng = SimpleRng::from_seed(424242);
        let sol = generate_full_solution(rng);
        let puzzle = _solution_to_string(&sol);

        let mut eng = Engine::new();
        add_all_sudoku_constraints(&mut eng);
        eng.load_givens(&puzzle).unwrap();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
        assert!(eng.has_unique_solution());
    }

    #[test]
    fn generate_puzzle_with_extra_constraints_stays_unique() {
        let extra = |e: &mut Engine| {
            add_kropki_white(e, (0, 0), (0, 1));
            add_kropki_black(e, (1, 0), (1, 1));
            add_thermo(e, &[(2, 2), (2, 3), (2, 4), (3, 4)]);
        };

        let rng = SimpleRng::from_seed(20240601);
        let target_clues = 35;
        let puzzle = generate_puzzle_with(target_clues, rng, extra);
        let clues = clue_count(&puzzle);
        assert!(
            clues >= target_clues,
            "expected at least {} clues, got {}",
            target_clues,
            clues
        );

        let mut eng = Engine::new();
        add_all_sudoku_constraints(&mut eng);
        extra(&mut eng);
        eng.load_givens(&puzzle).unwrap();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
        assert!(eng.has_unique_solution());
    }
}
