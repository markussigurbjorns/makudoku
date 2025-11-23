use crate::{CellIx, Contradiction, DIGITS_MASK, Domain, EVEN_MASK, State};

pub enum Constraint {
    AllDifferent { cells: [CellIx; 9] },
    KropkiWhite { a: CellIx, b: CellIx },
    KropkiBlack { a: CellIx, b: CellIx },
    Thermo { cells: Vec<CellIx> },
}

impl Constraint {
    pub fn scope<'a>(&'a self) -> Box<dyn Iterator<Item = CellIx> + 'a> {
        match self {
            Constraint::AllDifferent { cells } => Box::new(cells.iter().copied()),
            Constraint::KropkiWhite { a, b } => Box::new([*a, *b].into_iter()),
            Constraint::KropkiBlack { a, b } => Box::new([*a, *b].into_iter()),
            Constraint::Thermo { cells } => Box::new(cells.iter().copied()),
            // ARROW
            // KILLER
            // CHESS - KNIGHT
            // CHESS - KING
            // WHISPER LINES
            // USER DEFINED
        }
    }

    pub fn propagate(&self, state: &mut State) -> Result<bool, Contradiction> {
        match self {
            Constraint::AllDifferent { cells } => propagate_all_diff(state, cells),
            Constraint::KropkiWhite { a, b } => propagate_kropki_white(state, *a, *b),
            Constraint::KropkiBlack { a, b } => propagate_kropki_black(state, *a, *b),
            _ => {
                todo!()
            }
        }
    }
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

fn propagate_kropki_white(st: &mut State, a: CellIx, b: CellIx) -> Result<bool, Contradiction> {
    let da = st.domains[a as usize];
    let db = st.domains[b as usize];

    let reach_from_b = ((db << 1) | (db >> 1)) & DIGITS_MASK;
    let reach_from_a = ((da << 1) | (da >> 1)) & DIGITS_MASK;
    let mut changed = false;
    if st.narrow(a, reach_from_b)? {
        changed = true;
    }
    if st.narrow(b, reach_from_a)? {
        changed = true;
    }
    Ok(changed)
}

fn propagate_kropki_black(st: &mut State, a: CellIx, b: CellIx) -> Result<bool, Contradiction> {
    let da = st.domains[a as usize];
    let db = st.domains[b as usize];
    let not_allowed: [u8; 3] = [9, 7, 5];

    let mut reach_from_a: Domain = 0;
    let mut reach_from_b: Domain = 0;

    let mut ma = da;
    while ma != 0 {
        let d = ma.trailing_zeros() as u8; // 1..=9
        ma &= !(1u16 << d);
        if not_allowed.contains(&d) {
            continue;
        }
        if d < 5 {
            reach_from_a |= (1u16 << d * 2) & DIGITS_MASK;
        }
        if d % 2 == 0 {
            reach_from_a |= (1u16 << d / 2) & DIGITS_MASK;
        }
    }

    let mut mb = db;
    while mb != 0 {
        let d = mb.trailing_zeros() as u8; // 1..=9
        mb &= !(1u16 << d);
        if not_allowed.contains(&d) {
            continue;
        }
        if d < 5 {
            reach_from_b |= (1u16 << d * 2) & DIGITS_MASK;
        }
        if d % 2 == 0 {
            reach_from_b |= (1u16 << d / 2) & DIGITS_MASK;
        }
    }

    let mut changed = false;
    if st.narrow(a, reach_from_b)? {
        changed = true;
    }
    if st.narrow(b, reach_from_a)? {
        changed = true;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use crate::types::bit_of_digit;

    use super::*;

    fn mask(digits: &[u8]) -> Domain {
        digits.iter().fold(0, |acc, &d| acc | bit_of_digit(d))
    }

    #[test]
    fn test_all_diff_eliminates_taken_digits_from_peers() {
        let mut st = State::new();

        let cells: [CellIx; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

        // cell 0 is set to 5
        st.domains[0] = mask(&[5]);

        let changed = propagate_all_diff(&mut st, &cells).unwrap();
        assert!(changed);

        // cell 0 is still 5
        assert_eq!(st.domains[0], mask(&[5]));

        // all other cells do not contain 5 anymore
        let five = mask(&[5]);
        for &i in &cells[1..] {
            assert_eq!(st.domains[i as usize] & five, 0, "cell {} still has 5", i);
        }
    }

    #[test]
    fn test_all_diff_finds_hidden_single() {
        let mut st = State::new();

        let cells: [CellIx; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

        // Remove digit 9 from cells 0..7, so only cell 8 can be 9
        let nine = mask(&[9]);
        for i in 0..8 {
            st.domains[i] &= !nine;
        }

        let changed = propagate_all_diff(&mut st, &cells).unwrap();
        assert!(changed);

        // cell 8 must be 9
        assert_eq!(st.domains[8], nine);
    }

    #[test]
    fn all_diff_detects_duplicate_singleton_and_returns_contradiction() {
        let mut st = State::new();

        let cells: [CellIx; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

        // Cell 0 is single 3, cell 1 is also single 3 -> contradiction
        st.domains[0] = mask(&[3]);
        st.domains[1] = mask(&[3]);

        let res = propagate_all_diff(&mut st, &cells);
        assert!(res.is_err());
    }

    #[test]
    fn all_diff_no_change_when_already_consisdent() {
        let mut st = State::new();

        let cells: [CellIx; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

        // Give each cell a disjoint singleton (1..=9)
        for (i, d) in cells.iter().zip(1u8..=9) {
            st.domains[*i as usize] = mask(&[d]);
        }

        let changed = propagate_all_diff(&mut st, &cells).unwrap();
        assert!(!changed, "Should be already stable, no change expected");
    }

    #[test]
    fn kropki_white_limits_to_consecutive_digits() {
        let mut st = State::new();
        let a: CellIx = 0;
        let b: CellIx = 1;

        // a can be anything; b is fixed to 5
        st.domains[a as usize] = DIGITS_MASK;
        st.domains[b as usize] = mask(&[5]);

        let changed = propagate_kropki_white(&mut st, a, b).unwrap();
        assert!(changed);

        // a must now be {4, 6}; b stays 5
        assert_eq!(st.domains[a as usize], mask(&[4, 6]));
        assert_eq!(st.domains[b as usize], mask(&[5]));
    }

    #[test]
    fn kropki_white_is_symmetric() {
        let mut st = State::new();
        let a: CellIx = 0;
        let b: CellIx = 1;

        // a is set to {2,4,6,8}, b is fixed to 3
        st.domains[a as usize] = mask(&[2, 4, 6, 8]);
        st.domains[b as usize] = mask(&[3]);

        let _ = propagate_kropki_white(&mut st, a, b);

        // From b=3 we get {2,4} for a; from a={2,4,6,8} we get {3} for b
        assert_eq!(st.domains[a as usize], mask(&[2, 4]));
        assert_eq!(st.domains[b as usize], mask(&[3]));
    }

    #[test]
    fn kropki_white_detects_contradiction_when_no_consecutive_pair() {
        let mut st = State::new();
        let a: CellIx = 0;
        let b: CellIx = 1;

        // a is set to 1, b is set to 3 -> not consecutive
        st.domains[a as usize] = mask(&[1]);
        st.domains[b as usize] = mask(&[3]);

        let res = propagate_kropki_white(&mut st, a, b);
        assert!(
            res.is_err(),
            "expect contradiction for non consecutive pairs"
        );
    }

    #[test]
    fn kropki_black_enforces_ratio_two() {
        let mut st = State::new();
        let a: CellIx = 0;
        let b: CellIx = 1;

        // b is fixed to 3
        st.domains[a as usize] = DIGITS_MASK;
        st.domains[b as usize] = mask(&[4]);

        println!("domain a is {:}", st.domains[a as usize]);
        let changed = propagate_kropki_black(&mut st, a, b).unwrap();
        assert!(changed);

        assert_eq!(st.domains[a as usize], mask(&[2, 8]));
        println!("domain a is {:}", st.domains[a as usize]);
        assert_eq!(st.domains[b as usize], mask(&[4]));
    }
}
