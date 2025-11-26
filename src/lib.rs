#![allow(clippy::redundant_pub_crate)]
mod analysis;
mod constraints;
mod engine;
mod generator;
mod render;
mod state;
mod types;

pub use analysis::{estimate_difficulty, estimate_difficulty_with};
pub use constraints::Constraint;
pub use engine::{
    Engine, add_all_sudoku_constraints, add_kropki_black, add_kropki_white, add_thermo,
};
pub use generator::{
    generate_full_solution, generate_full_solution_with, generate_puzzle, generate_puzzle_with,
};
pub use state::State;
pub use types::{
    CellIx, Contradiction, DIGITS_MASK, Difficulty, Domain, EVEN_MASK, N, NN, Solve, bit_of_digit,
    box_of, col_of, digit_of_bit, row_of,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solve_empty() {
        let mut eng = Engine::new();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
    }

    #[test]
    fn solve_classic() {
        let p = "2...7.1.3.7..8..5.3....6.....6......91..5..28......5.....3....4.2..9..7.5.4.1...6";
        let mut eng = Engine::new();
        add_all_sudoku_constraints(&mut eng);
        eng.load_givens(p).unwrap();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
        assert!(eng.has_unique_solution());
        assert_eq!(eng.count_solutions(10), 1);
    }

    #[test]
    fn solve_kropki_white_only() {
        let p = "...7....4.1.........6......4...........3.7...........8......7.........8.3....2...";
        let mut eng = Engine::new();
        add_all_sudoku_constraints(&mut eng);
        add_kropki_white(&mut eng, (6, 1), (7, 1));
        add_kropki_white(&mut eng, (3, 1), (3, 2));
        add_kropki_white(&mut eng, (7, 1), (7, 2));
        add_kropki_white(&mut eng, (3, 2), (3, 3));
        add_kropki_white(&mut eng, (1, 3), (2, 3));
        add_kropki_white(&mut eng, (2, 3), (3, 3));
        add_kropki_white(&mut eng, (5, 5), (6, 5));
        add_kropki_white(&mut eng, (6, 5), (7, 5));
        add_kropki_white(&mut eng, (1, 6), (1, 7));
        add_kropki_white(&mut eng, (5, 6), (5, 7));
        add_kropki_white(&mut eng, (1, 7), (2, 7));
        add_kropki_white(&mut eng, (5, 5), (5, 6));
        eng.load_givens(p).unwrap();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
        assert!(eng.has_unique_solution());
    }

    #[test]
    fn solve_kropki() {
        let p = "...........12......3..7.....6..5......84....................123......456......789";
        let mut eng = Engine::new();
        add_all_sudoku_constraints(&mut eng);
        add_kropki_white(&mut eng, (0, 2), (0, 3));
        add_kropki_white(&mut eng, (1, 0), (1, 1));
        add_kropki_white(&mut eng, (1, 4), (1, 5));
        add_kropki_white(&mut eng, (4, 0), (4, 1));
        add_kropki_white(&mut eng, (4, 4), (4, 5));
        add_kropki_white(&mut eng, (5, 2), (5, 3));

        add_kropki_black(&mut eng, (0, 1), (1, 1));
        add_kropki_black(&mut eng, (0, 4), (1, 4));
        add_kropki_black(&mut eng, (2, 0), (3, 0));
        add_kropki_black(&mut eng, (2, 5), (3, 5));
        add_kropki_black(&mut eng, (4, 1), (5, 1));
        add_kropki_black(&mut eng, (4, 4), (5, 4));
        eng.load_givens(p).unwrap();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
        assert!(eng.has_unique_solution());
    }

    #[test]
    fn solve_thermo() {
        let p = "....4......175...........4........9.63.....25.8........1...........759......6....";
        let mut eng = Engine::new();
        add_all_sudoku_constraints(&mut eng);
        add_thermo(&mut eng, &[(0, 5), (1, 5), (2, 5), (3, 5)]);
        add_thermo(&mut eng, &[(0, 6), (1, 6), (2, 6), (3, 6)]);
        add_thermo(&mut eng, &[(2, 0), (2, 1), (2, 2), (2, 3)]);
        add_thermo(&mut eng, &[(3, 0), (3, 1), (3, 2), (3, 3)]);
        add_thermo(&mut eng, &[(5, 8), (5, 7), (5, 6), (5, 5)]);
        add_thermo(&mut eng, &[(6, 8), (6, 7), (6, 6), (6, 5)]);
        add_thermo(&mut eng, &[(8, 2), (7, 2), (6, 2), (5, 2)]);
        add_thermo(&mut eng, &[(8, 3), (7, 3), (6, 3), (5, 3)]);
        eng.load_givens(p).unwrap();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
        assert!(eng.has_unique_solution());
    }
}
