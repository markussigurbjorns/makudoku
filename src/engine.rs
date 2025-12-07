use std::collections::VecDeque;

use crate::{
    CellIx, Constraint, Contradiction, Domain, N, NN, Solve, State,
    types::{bit_of_digit, idx},
};

#[derive(Clone, Debug)]
pub struct Engine {
    pub state: State,
    pub constraints: Vec<Constraint>,
    pub branches: u32,
    watchers: Vec<Vec<usize>>,
    queued: Vec<bool>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            state: State::new(),
            constraints: Vec::new(),
            watchers: vec![Vec::new(); NN],
            branches: 0,
            queued: Vec::new(),
        }
    }
    fn save_state(&self) -> ([Domain; NN], Vec<(CellIx, Domain)>, VecDeque<usize>, u32) {
        (
            self.state.domains,
            self.state.trail.clone(),
            self.state.queue.clone(),
            self.branches,
        )
    }

    fn restore_state(&mut self, snap: ([Domain; NN], Vec<(CellIx, Domain)>, VecDeque<usize>, u32)) {
        self.state.domains = snap.0;
        self.state.trail = snap.1;
        self.state.queue = snap.2;
        self.branches = snap.3;
        self.queued.fill(false);
        for &ci in &self.state.queue {
            self.queued[ci] = true;
        }
    }

    pub fn with_saved_state<T>(&mut self, f: impl FnOnce(&mut Engine) -> T) -> T {
        let snap = self.save_state();
        let out = f(self);
        self.restore_state(snap);
        out
    }

    pub fn add_constraint(&mut self, c: Constraint) {
        let idx = self.constraints.len();
        c.for_each_cell(|i| {
            self.watchers[i as usize].push(idx);
        });
        self.constraints.push(c);
        self.queued.push(false);
    }

    pub fn enqueue_all(&mut self) {
        for i in 0..self.constraints.len() {
            self.enqueue_constraint(i);
        }
    }

    pub fn enqueue_cell_constraints(&mut self, i: CellIx) {
        let watchers = self.watchers[i as usize].clone();
        for ci in watchers {
            self.enqueue_constraint(ci);
        }
    }

    #[inline]
    fn enqueue_constraint(&mut self, ci: usize) {
        if !self.queued[ci] {
            self.queued[ci] = true;
            self.state.queue.push_back(ci);
        }
    }

    pub fn propagate(&mut self) -> Result<Solve, Contradiction> {
        let mut any = false;
        while let Some(ci) = self.state.queue.pop_front() {
            self.queued[ci] = false;
            let changed = self.constraints[ci].propagate(&mut self.state)?;
            if changed {
                any = true;
                let mut cells = Vec::new();
                self.constraints[ci].for_each_cell(|j| {
                    cells.push(j);
                });
                for j in cells {
                    let watchers = self.watchers[j as usize].clone();
                    for c2 in watchers {
                        self.enqueue_constraint(c2);
                    }
                }
            }
        }
        if any {
            Ok(Solve::Progress)
        } else {
            Ok(Solve::Stalled)
        }
    }

    pub fn load_givens(&mut self, s: &str) -> Result<(), String> {
        let bytes: Vec<u8> = s
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .map(|ch| ch as u8)
            .collect();
        if bytes.len() != NN {
            return Err(format!("need 81 chars, got {}", bytes.len()));
        }
        for (i, &ch) in bytes.iter().enumerate() {
            if ch == b'.' || ch == b'0' {
                continue;
            }
            if !(b'1'..=b'9').contains(&ch) {
                return Err(format!("invalid char at {}: {}", i, ch as char));
            }
            let mask = bit_of_digit((ch - b'0') as u8);
            let i = i as CellIx;
            self.state
                .assign(i, mask)
                .map_err(|_| "contradiction from givens".to_string())?;
            self.enqueue_cell_constraints(i);
        }
        match self.propagate() {
            Ok(_) => Ok(()),
            Err(_) => Err("contradiction from givens".into()),
        }
    }

    pub fn solved(&self) -> bool {
        self.state.domains.iter().all(|&m| m.count_ones() == 1)
    }

    pub fn choose_mrv(&self) -> Option<CellIx> {
        let mut best: Option<(CellIx, u32)> = None;
        for i in 0..NN {
            let m = self.state.domains[i];
            let cnt = m.count_ones();
            if cnt > 1 {
                match best {
                    None => best = Some((i as CellIx, cnt)),
                    Some((_, best_cnt)) if cnt < best_cnt => best = Some((i as CellIx, cnt)),
                    _ => {}
                }
            }
        }
        best.map(|(i, _)| i)
    }

    pub fn search(&mut self) -> Result<bool, Contradiction> {
        // enqueue all only at root
        if self.state.trail.is_empty() && self.state.queue.is_empty() {
            self.enqueue_all();
        }

        // First, propagate as far as we can
        loop {
            match self.propagate() {
                Ok(res) => match res {
                    Solve::Progress => {
                        if self.solved() {
                            return Ok(true);
                        }
                    }
                    Solve::Solved | Solve::Stalled => break,
                },
                Err(_) => {
                    // contradiction at this node
                    return Ok(false);
                }
            }
        }

        if self.solved() {
            return Ok(true);
        }

        if self.state.domains.iter().any(|&m| m == 0) {
            return Ok(false);
        }

        // pick MRV cell
        let i = match self.choose_mrv() {
            None => {
                // all singletons => solved
                return Ok(true);
            }
            Some(i) => i,
        };
        let dom = self.state.domains[i as usize];

        // branch over its values
        let trail_len = self.state.trail.len();
        let mut m = dom;
        while m != 0 {
            let d = m.trailing_zeros() as u8;
            let bit = bit_of_digit(d);
            m &= !bit;

            self.branches += 1;

            if self.state.assign(i, bit).is_ok() {
                self.enqueue_cell_constraints(i);
                let res = self.search();
                match res {
                    Ok(true) => {
                        // found a solution; DO NOT backtrack it away
                        return Ok(true);
                    }
                    Ok(false) => {
                        // try next digit
                    }
                    Err(Contradiction) => {
                        // also just try next digit
                    }
                }
            }
            // undo this branch
            self.state.backtrack_to(trail_len);
        }

        Ok(false)
    }

    pub fn count_solutions(&self, max: u32) -> u32 {
        let mut eng = self.clone();

        if eng.state.trail.is_empty() && eng.state.queue.is_empty() {
            eng.enqueue_all();
        }

        let mut count = 0;
        eng.search_limited(max, &mut count);
        count
    }

    pub fn has_unique_solution(&mut self) -> bool {
        self.count_solutions(2) == 1
    }

    fn search_limited(&mut self, max: u32, count: &mut u32) {
        if *count >= max {
            return;
        }

        // propagate
        loop {
            match self.propagate() {
                Ok(Solve::Progress) => {
                    if self.solved() {
                        *count += 1;
                        return;
                    }
                }
                Ok(Solve::Stalled) | Ok(Solve::Solved) => break,
                Err(Contradiction) => {
                    // dead branch
                    return;
                }
            }
        }

        if self.solved() {
            *count += 1;
            return;
        }

        if self.state.domains.iter().any(|&m| m == 0) {
            return;
        }

        if *count >= max {
            return;
        }

        let i = match self.choose_mrv() {
            None => {
                // all singletons, another solution
                *count += 1;
                return;
            }
            Some(i) => i,
        };

        let dom = self.state.domains[i as usize];
        let trail_len = self.state.trail.len();
        let mut m = dom;

        while m != 0 && *count < max {
            let d = m.trailing_zeros() as u8;
            let bit = bit_of_digit(d);
            m &= !bit;

            self.branches += 1;

            if self.state.assign(i, bit).is_ok() {
                self.enqueue_cell_constraints(i);
                self.search_limited(max, count);
            }

            self.state.backtrack_to(trail_len);
        }
    }
}

pub fn add_all_sudoku_constraints(e: &mut Engine) {
    for r in 0..N {
        let mut cells = [0u8; 9];
        for c in 0..N {
            cells[c] = idx(r, c);
        }
        e.add_constraint(Constraint::AllDifferent { cells, len: 9 });
    }

    for c in 0..N {
        let mut cells = [0u8; 9];
        for r in 0..N {
            cells[r] = idx(r, c);
        }
        e.add_constraint(Constraint::AllDifferent { cells, len: 9 });
    }

    for br in 0..3 {
        for bc in 0..3 {
            let mut cells = [0u8; 9];
            let mut k = 0;
            for dr in 0..3 {
                for dc in 0..3 {
                    cells[k] = idx(br * 3 + dr, bc * 3 + dc);
                    k += 1;
                }
            }
            e.add_constraint(Constraint::AllDifferent { cells, len: 9 });
        }
    }
}

pub fn add_kropki_white(e: &mut Engine, a_rc: (usize, usize), b_rc: (usize, usize)) {
    let a = idx(a_rc.0, a_rc.1);
    let b = idx(b_rc.0, b_rc.1);
    e.add_constraint(Constraint::KropkiWhite { a, b });
}

pub fn add_kropki_black(e: &mut Engine, a_rc: (usize, usize), b_rc: (usize, usize)) {
    let a = idx(a_rc.0, a_rc.1);
    let b = idx(b_rc.0, b_rc.1);
    e.add_constraint(Constraint::KropkiBlack { a, b });
}

pub fn add_thermo(e: &mut Engine, cells_rc: &[(usize, usize)]) {
    assert!(!cells_rc.is_empty(), "thermo must have at least 1 cell");
    assert!(
        cells_rc.len() <= 9,
        "thermo longer than 9 cells is not supported"
    );

    let mut cells_arr = [0u8; 9];
    for (i, &(r, c)) in cells_rc.iter().enumerate() {
        cells_arr[i] = idx(r, c);
    }
    let len = cells_rc.len() as u8;

    e.add_constraint(Constraint::Thermo {
        cells: cells_arr,
        len,
    });
}

pub fn add_arrow(e: &mut Engine, cells_rc: &[(usize, usize)]) {
    let mut cells_arr = [0u8; 9];
    for (i, &(r, c)) in cells_rc.iter().enumerate() {
        cells_arr[i] = idx(r, c);
    }
    let len = cells_rc.len() as u8;

    e.add_constraint(Constraint::Arrow {
        cells: cells_arr,
        len,
    });
}

pub fn add_king_constraints(e: &mut Engine) {
    const KING_DELTAS: &[(i32, i32)] = &[
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];

    for r in 0..N {
        for c in 0..N {
            let a = idx(r, c);
            for &(dr, dc) in KING_DELTAS {
                let nr = r as i32 + dr;
                let nc = c as i32 + dc;
                if nr < 0 || nr >= N as i32 || nc < 0 || nc >= N as i32 {
                    continue;
                }
                let b = idx(nr as usize, nc as usize);
                // Avoid duplicate constraints (only add when a < b)
                if a < b {
                    e.add_constraint(Constraint::King { a, b });
                }
            }
        }
    }
}

pub fn add_knight_constraints(e: &mut Engine) {
    const KNIGHT_DELTAS: &[(i32, i32)] = &[
        (-2, -1),
        (-2, 1),
        (-1, -2),
        (-1, 2),
        (1, -2),
        (1, 2),
        (2, -1),
        (2, 1),
    ];

    for r in 0..N {
        for c in 0..N {
            let a = idx(r, c);
            for &(dr, dc) in KNIGHT_DELTAS {
                let nr = r as i32 + dr;
                let nc = c as i32 + dc;
                if nr < 0 || nr >= N as i32 || nc < 0 || nc >= N as i32 {
                    continue;
                }
                let b = idx(nr as usize, nc as usize);
                if a < b {
                    e.add_constraint(Constraint::Knight { a, b });
                }
            }
        }
    }
}

pub fn add_queen_constraints(e: &mut Engine) {
    const DIAGS: &[(i32, i32)] = &[(-1, -1), (-1, 1), (1, -1), (1, 1)];

    for r in 0..N {
        for c in 0..N {
            let a = idx(r, c);
            for &(dr, dc) in DIAGS {
                let mut nr = r as i32 + dr;
                let mut nc = c as i32 + dc;

                while nr >= 0 && nr < N as i32 && nc >= 0 && nc < N as i32 {
                    let b = idx(nr as usize, nc as usize);

                    // avoid duplicate constraints
                    if a < b {
                        e.add_constraint(Constraint::Queen { a, b });
                    }

                    nr += dr;
                    nc += dc;
                }
            }
        }
    }
}

pub fn add_killer_cage(e: &mut Engine, cells_rc: &[(usize, usize)], sum: u8, no_repeats: bool) {
    assert!(
        !cells_rc.is_empty(),
        "killer cage must have at least 1 cell"
    );
    assert!(
        cells_rc.len() <= 9,
        "killer cage longer than 9 cells is not supported"
    );

    let mut cells_arr = [0u8; 9];
    for (i, &(r, c)) in cells_rc.iter().enumerate() {
        cells_arr[i] = idx(r, c);
    }
    let len = cells_rc.len() as u8;

    if no_repeats && cells_rc.len() > 1 {
        let mut ad_cells = [0u8; 9];
        for (i, &cell) in cells_arr.iter().enumerate().take(cells_rc.len()) {
            ad_cells[i] = cell;
        }
        e.add_constraint(Constraint::AllDifferent {
            cells: ad_cells,
            len: len as u8,
        });
    }

    e.add_constraint(Constraint::Killer {
        cells: cells_arr,
        len,
        sum,
    });
}

#[cfg(test)]
mod tests {
    use std::usize;

    use super::*;
    use crate::{DIGITS_MASK, Domain, types::bit_of_digit};

    fn mask(digits: &[u8]) -> Domain {
        digits.iter().fold(0, |acc, &d| acc | bit_of_digit(d))
    }

    #[test]
    fn new_initalizes_state_and_watchers() {
        let eng = Engine::new();

        // no constraints yet
        assert!(eng.constraints.is_empty());

        // watchers for all NN cells
        assert_eq!(eng.watchers.len(), NN);
        assert!(eng.watchers.iter().all(|v| v.is_empty()));

        // state initialized normally
        for d in eng.state.domains.iter() {
            assert_eq!(*d, DIGITS_MASK)
        }
        assert!(eng.state.trail.is_empty());
        assert!(eng.state.queue.is_empty());
    }

    #[test]
    fn add_constraint_registers_watchers_for_cells_in_scope() {
        let mut eng = Engine::new();
        let cells: [CellIx; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

        // all different first row
        let c = Constraint::AllDifferent { cells, len: 9 };
        eng.add_constraint(c);

        // ensure one constraint
        assert_eq!(eng.constraints.len(), 1);

        // cells in first row should have watcher 0
        for c in 0..N {
            let cell_ix = idx(0, c) as usize;
            assert_eq!(
                eng.watchers[cell_ix],
                vec![0],
                "cell {} missing watcher",
                cell_ix
            )
        }

        // other cells not in row 0 should have no wathcers
        let other = idx(1, 0) as usize;
        assert!(eng.watchers[other].is_empty());
    }

    #[test]
    fn enqueue_all_pushes_all_constraints_into_queue() {
        let mut eng = Engine::new();

        // Add two fake AllDifferent constraints on two rows
        for r in 0..2 {
            let mut cells = [0u8; 9];
            for c in 0..9 {
                cells[c] = idx(r, c);
            }
            eng.add_constraint(Constraint::AllDifferent { cells, len: 9 });
        }

        eng.enqueue_all();

        // We expect indices [0, 1] in the queue (order not hugely important)
        assert_eq!(eng.state.queue.len(), 2);
        let items: Vec<usize> = eng.state.queue.iter().copied().collect();
        assert!(items.contains(&0));
        assert!(items.contains(&1));
    }

    #[test]
    fn enqueue_cell_constraints_uses_watchers() {
        let mut eng = Engine::new();

        // one constraint covering first row
        let cells: [CellIx; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
        eng.add_constraint(Constraint::AllDifferent { cells, len: 9 });

        // queue initially empty
        assert!(eng.state.queue.is_empty());

        // enqueu constraints for cell (0,3)
        let cell = idx(0, 3);
        eng.enqueue_cell_constraints(cell);

        assert_eq!(eng.state.queue.len(), 1);
        assert_eq!(eng.state.queue.pop_front(), Some(0));
    }

    #[test]
    fn add_killer_cage_uses_len_for_all_diff_scope() {
        let mut eng = Engine::new();
        add_killer_cage(&mut eng, &[(0, 0), (0, 1)], 7, true);

        assert_eq!(eng.constraints.len(), 2);
        assert!(matches!(
            eng.constraints[0],
            Constraint::AllDifferent { len, .. } if len == 2
        ));
        assert!(matches!(
            eng.constraints[1],
            Constraint::Killer { len, sum, .. } if len == 2 && sum == 7
        ));
    }

    #[test]
    fn propagate_runs_constraint_and_clears_queue() {
        let mut eng = Engine::new();

        // all different on first row
        let cells: [CellIx; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
        eng.add_constraint(Constraint::AllDifferent { cells, len: 9 });

        // make cell (0,0) a singleton 1, others {1,2,3}
        let c00 = idx(0, 0) as usize;
        eng.state.domains[c00] = mask(&[1]);
        for c in 1..N {
            let ix = idx(0, c) as usize;
            eng.state.domains[ix] = mask(&[1, 2, 3]);
        }

        // enqueue all and propagate
        eng.enqueue_all();
        let res = eng.propagate().unwrap();
        assert_eq!(res, Solve::Progress);

        // all row cells except (0,0) should no longer contain 1
        let one = mask(&[1]);
        for c in 1..N {
            let ix = idx(0, c) as usize;
            assert_eq!(
                eng.state.domains[ix] & one,
                0,
                "cell (0,{}) still has digit 1",
                c
            );
        }

        // queue should now be empty
        assert!(eng.state.queue.is_empty());
    }

    #[test]
    fn load_givens_rejects_wrong_length() {
        let mut eng = Engine::new();
        let err = eng.load_givens("123").unwrap_err();
        assert!(err.contains("need 81 chars"), "unexpected error: {}", err);
    }

    #[test]
    fn load_givens_rejects_invalid_char() {
        let mut eng = Engine::new();
        // 80 dots + 'x'
        let mut s = ".".repeat(NN - 1);
        s.push('x');

        let err = eng.load_givens(&s).unwrap_err();
        assert!(err.contains("invalid char"), "unexpected error: {}", err);
    }

    #[test]
    fn load_givens_sets_singletons_for_digits() {
        let mut eng = Engine::new();

        // "1" in first cell, rest dots
        let mut s = String::new();
        s.push('1');
        s.push_str(&".".repeat(NN - 1));

        eng.load_givens(&s).unwrap();

        // first cell should be exactly digit 1
        assert_eq!(eng.state.domains[0], mask(&[1]));

        // some other cell still has full domain (no constraints added)
        assert_eq!(eng.state.domains[1], DIGITS_MASK);
    }

    #[test]
    fn load_givens_errors_on_assign_contradiction() {
        let mut eng = Engine::new();

        // manually restrict cell 0 to digit 1
        eng.state.domains[0] = mask(&[1]);

        // but givens say '2' in that cell
        let mut s = String::new();
        s.push('2');
        s.push_str(&".".repeat(NN - 1));

        let err = eng.load_givens(&s).unwrap_err();
        assert_eq!(err, "contradiction from givens");
    }

    #[test]
    fn solved_true_when_all_singletons() {
        let mut eng = Engine::new();

        for d in eng.state.domains.iter_mut() {
            *d = mask(&[1]);
        }

        assert!(eng.solved());
    }

    #[test]
    fn solved_false_when_any_multi_domain() {
        let mut eng = Engine::new();

        for d in eng.state.domains.iter_mut() {
            *d = mask(&[1]);
        }
        // make one cell have 2 possibilities
        eng.state.domains[10] = mask(&[1, 2]);

        assert!(!eng.solved());
    }
    #[test]
    fn choose_mrv_picks_smallest_non_singleton_domain() {
        let mut eng = Engine::new();

        // all singletons
        for d in eng.state.domains.iter_mut() {
            *d = mask(&[1]);
        }

        // cell 5: two values, cell 7: three values
        eng.state.domains[5] = mask(&[1, 2]);
        eng.state.domains[7] = mask(&[1, 2, 3]);

        let mrv = eng.choose_mrv().unwrap();
        assert_eq!(mrv, 5); // cell 5 should win (2 < 3)

        // make cell 5 singleton again; now 7 is the only non-singleton
        eng.state.domains[5] = mask(&[1]);
        let mrv2 = eng.choose_mrv().unwrap();
        assert_eq!(mrv2, 7);

        // all singletons -> None
        eng.state.domains[7] = mask(&[1]);
        assert!(eng.choose_mrv().is_none());
    }

    #[test]
    fn search_returns_true_when_already_solved() {
        let mut eng = Engine::new();

        // all domains singletons; no constraints
        for d in eng.state.domains.iter_mut() {
            *d = mask(&[1]);
        }

        let res = eng.search();
        assert_eq!(res, Ok(true));
    }

    #[test]
    fn search_returns_false_if_any_domain_empty() {
        let mut eng = Engine::new();

        // make one domain 0 (contradiction), rest arbitrary
        eng.state.domains[0] = 0;

        let res = eng.search();
        assert_eq!(res, Ok(false));
    }
}
