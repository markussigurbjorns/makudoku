use std::collections::VecDeque;

pub const N: usize = 9;
pub const NN: usize = N * N; //81

pub type CellIx = u8; //0..80
pub type Domain = u16; //bits 1..=9 used

pub const DIGITS_MASK: Domain = 0b_11_1111_1110;

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
fn idx(r: usize, c: usize) -> CellIx {
    (r * N + c) as CellIx
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Solve {
    Solved,
    Progress,
    Stalled,
}

#[derive(Debug)]
pub struct Contradiction;

#[derive(Clone)]
pub struct State {
    pub domains: [Domain; NN],
    trail: Vec<(CellIx, Domain)>,
    queue: VecDeque<usize>,
}

impl State {
    pub fn new() -> Self {
        Self {
            domains: [DIGITS_MASK; NN],
            trail: Vec::with_capacity(256),
            queue: VecDeque::new(),
        }
    }

    pub fn narrow(&mut self, i: CellIx, mask: Domain) -> Result<bool, Contradiction> {
        let di = &mut self.domains[i as usize];
        let old = *di;
        let new = old & mask;
        if new == 0 {
            return Err(Contradiction);
        }
        if new != old {
            self.trail.push((i, old));
            *di = new;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn assign(&mut self, i: CellIx, single: Domain) -> Result<bool, Contradiction> {
        debug_assert!(single.count_ones() == 1);
        self.narrow(i, single)
    }

    pub fn backtrack_to(&mut self, trail_len: usize) {
        while self.trail.len() > trail_len {
            let (i, old) = self.trail.pop().unwrap();
            self.domains[i as usize] = old;
        }
    }
}

pub enum Constraint {
    AllDifferent { cells: [CellIx; 9] },
    KropkiWhite { a: CellIx, b: CellIx },
    Thermo { cells: Vec<CellIx> },
}

impl Constraint {
    pub fn scope<'a>(&'a self) -> Box<dyn Iterator<Item = CellIx> + 'a> {
        match self {
            Constraint::AllDifferent { cells } => Box::new(cells.iter().copied()),
            Constraint::KropkiWhite { a, b } => Box::new([*a, *b].into_iter()),
            Constraint::Thermo { cells } => Box::new(cells.iter().copied()),
        }
    }

    pub fn propagate(&self, state: &mut State) -> Result<bool, Contradiction> {
        match self {
            Constraint::AllDifferent { cells } => propagate_all_diff(state, cells),
            _ => {
                todo!()
            }
        }
    }
}

#[inline]
fn bit_of_digit(d: u8) -> Domain {
    1u16 << d
}

fn propagate_all_diff(st: &mut State, cells: &[CellIx; 9]) -> Result<bool, Contradiction> {
    let mut changed = false;

    let mut taken: Domain = 0;
    let mut count: [u8; 10] = [0; 10]; // count[d] for d in 1..=9
    let mut last_pos: [Option<CellIx>; 10] = [None; 10];

    for &i in cells.iter() {
        let di = st.domains[i as usize];
        if di == 0 {
            return Err(Contradiction);
        }

        let mut m = di;
        while m != 0 {
            let d = m.trailing_zeros() as u8; // 1..=9
            m &= !(1u16 << d);
            count[d as usize] += 1;
            last_pos[d as usize] = Some(i);
        }

        if di.count_ones() == 1 {
            taken |= di;
        }
    }

    for d in 1..=9 {
        if count[d as usize] >= 2 {
            let bit = 1u16 << d;
            let mut singles_with_d = 0u8;
            for &i in cells.iter() {
                if st.domains[i as usize] == bit {
                    singles_with_d += 1;
                    if singles_with_d >= 2 {
                        return Err(Contradiction);
                    }
                }
            }
        }
    }

    for d in 1..=9 {
        if count[d as usize] == 1 {
            let bit = 1u16 << d;
            let i = last_pos[d as usize].unwrap();
            if st.assign(i, bit)? {
                changed = true;
            }
        }
    }

    if taken != 0 {
        for &i in cells.iter() {
            let di = st.domains[i as usize];
            if di.count_ones() == 1 {
                continue;
            }
            let mask = di & !taken;
            if st.narrow(i, mask)? {
                changed = true;
            }
        }
    }

    Ok(changed)
}

pub struct Engine {
    pub state: State,
    pub constraints: Vec<Constraint>,
    watchers: Vec<Vec<usize>>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            state: State::new(),
            constraints: Vec::new(),
            watchers: vec![Vec::new(); NN],
        }
    }

    pub fn add_constraints(&mut self, c: Constraint) {
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
            match self.propagate()? {
                Solve::Progress => {
                    if self.solved() {
                        return Ok(true);
                    }
                }
                Solve::Solved => break,
                Solve::Stalled => break,
            }
        }
        if self.solved() {
            return Ok(true);
        }

        if self.state.domains.iter().any(|&m| m == 0) {
            return Err(Contradiction);
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

            // try branch
            if self.state.assign(i, bit).is_ok() {
                self.enqueue_cell_constraints(i);
                if self.search()? {
                    return Ok(true);
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
        e.add_constraints(Constraint::AllDifferent { cells });
    }

    for c in 0..N {
        let mut cells = [0u8; 9];
        for r in 0..N {
            cells[r] = idx(r, c);
        }
        e.add_constraints(Constraint::AllDifferent { cells });
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
            e.add_constraints(Constraint::AllDifferent { cells });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_classic() {
        let p = "7.4..6..9.8..1......3.2.45.........2.56...78.1.........25.3.1......4..6.9..5..3.7";
        let mut eng = Engine::new();
        add_all_sudoku_constraints(&mut eng);
        eng.load_givens(p).unwrap();
        assert!(eng.search().unwrap());
        assert!(eng.solved());
    }

    #[test]
    fn test_bit_of_digit() {
        let res = bit_of_digit(4);
        assert_eq!(res, 0b1_0000);
    }
}
