use crate::{Difficulty, Engine, add_all_sudoku_constraints};

pub fn estimate_difficulty_with<F>(puzzle: &str, extra: F) -> Result<Difficulty, String>
where
    F: FnOnce(&mut Engine) + Clone,
{
    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);
    extra(&mut eng);
    eng.load_givens(puzzle)?;

    // pure propagation loop (no branching)
    let mut logical_steps = 0usize;
    loop {
        let before = eng.state.domains;
        match eng.propagate() {
            Err(_) => return Err("unsatisfiable".into()),
            Ok(crate::Solve::Progress) => {
                logical_steps += 1;
            }
            Ok(crate::Solve::Stalled) | Ok(crate::Solve::Solved) => {}
        }

        if eng.solved() {
            // solved by propagation only
            return Ok(if logical_steps <= 2 {
                Difficulty::Trivial
            } else {
                Difficulty::Easy
            });
        }

        if eng.state.domains == before {
            break;
        }
    }

    // backtracking: measure branches
    let b = eng.with_saved_state(|eng2| {
        eng2.branches = 0;
        let ok = eng2.search().map_err(|_| "unsatisfiable".to_string())?;
        if !ok || !eng2.solved() {
            return Err("unsatisfiable".to_string());
        }
        Ok(eng2.branches)
    })?;

    let diff = if b < 10 {
        Difficulty::Medium
    } else if b < 100 {
        Difficulty::Hard
    } else {
        Difficulty::Insane
    };

    Ok(diff)
}

pub fn estimate_difficulty(puzzle: &str) -> Result<Difficulty, String> {
    estimate_difficulty_with(puzzle, |_: &mut Engine| {})
}
