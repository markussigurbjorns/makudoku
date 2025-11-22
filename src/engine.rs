use crate::{
    CellIx, Constraint, Contradiction, N, NN, Solve, State,
    types::{bit_of_digit, idx},
};

pub struct Engine {
    pub state: State,
    pub constraints: Vec<Constraint>,
    watchers: Vec<Vec<usize>>,
    branches: u32,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            state: State::new(),
            constraints: Vec::new(),
            watchers: vec![Vec::new(); NN],
            branches: 0,
        }
    }

    pub fn add_constraint(&mut self, c: Constraint) {
        let idx = self.constraints.len();
        for i in c.scope() {
            self.watchers[i as usize].push(idx);
        }
        self.constraints.push(c);
    }

    pub fn enqueue_all(&mut self) {
        for i in 0..self.constraints.len() {
            self.state.queue.push_back(i);
        }
    }

    pub fn enqueue_cell_constraints(&mut self, i: CellIx) {
        for &ci in &self.watchers[i as usize] {
            self.state.queue.push_back(ci);
        }
    }

    pub fn propagate(&mut self) -> Result<Solve, Contradiction> {
        let mut any = false;
        while let Some(ci) = self.state.queue.pop_front() {
            let changed = self.constraints[ci].propagate(&mut self.state)?;
            if changed {
                any = true;
                // Re-enqueue neighbors: every cell in this constraint
                for j in self.constraints[ci].scope() {
                    for &c2 in &self.watchers[j as usize] {
                        self.state.queue.push_back(c2);
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
    /// Initialize from givens: digits string of length 81 ('.' or '0' for blank).
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

    /// Choose MRV cell (domain size >1 with minimal count). Returns None if all singletons.
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
        loop {
            match self.propagate() {
                Ok(res) => match res {
                    Solve::Progress => {
                        if self.solved() {
                            return Ok(true);
                        }
                    }
                    Solve::Solved => break,
                    Solve::Stalled => break,
                },
                Err(_) => {
                    return Ok(false);
                }
            }

            {}
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
            // try branch
            if self.state.assign(i, bit).is_ok() {
                self.enqueue_cell_constraints(i);
                let res = self.search();
                match res {
                    Ok(true) => {
                        return Ok(true);
                    }
                    Ok(false) => {
                        // branch failed, try next digit
                    }
                    Err(Contradiction) => {
                        // branch failed, try next digit
                    }
                }
            }
            self.state.backtrack_to(trail_len);
        }

        Ok(false)
    }
}

pub fn add_all_sudoku_constraints(e: &mut Engine) {
    for r in 0..N {
        let mut cells = [0u8; 9];
        for c in 0..N {
            cells[c] = idx(r, c);
        }
        e.add_constraint(Constraint::AllDifferent { cells });
    }

    for c in 0..N {
        let mut cells = [0u8; 9];
        for r in 0..N {
            cells[r] = idx(r, c);
        }
        e.add_constraint(Constraint::AllDifferent { cells });
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
            e.add_constraint(Constraint::AllDifferent { cells });
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
