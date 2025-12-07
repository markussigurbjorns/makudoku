use std::usize;

use crate::{CellIx, Contradiction, DIGITS_MASK, Domain, State};

#[derive(Clone, Debug)]
pub enum Constraint {
    AllDifferent {
        cells: [CellIx; 9],
        len: u8,
    },
    KropkiWhite {
        a: CellIx,
        b: CellIx,
    },
    KropkiBlack {
        a: CellIx,
        b: CellIx,
    },
    Thermo {
        cells: [CellIx; 9],
        len: u8,
    },
    Arrow {
        cells: [CellIx; 9],
        len: u8,
    },
    King {
        a: CellIx,
        b: CellIx,
    },
    Knight {
        a: CellIx,
        b: CellIx,
    },
    Queen {
        a: CellIx,
        b: CellIx,
    },
    Killer {
        cells: [CellIx; 9],
        len: u8,
        sum: u8,
    },
}

impl Constraint {
    #[inline]
    pub fn for_each_cell<F>(&self, mut f: F)
    where
        F: FnMut(CellIx),
    {
        match self {
            Constraint::AllDifferent { cells, len } => {
                let len = *len as usize;
                for &c in cells.iter().take(len) {
                    f(c);
                }
            }
            Constraint::KropkiWhite { a, b }
            | Constraint::KropkiBlack { a, b }
            | Constraint::King { a, b }
            | Constraint::Queen { a, b }
            | Constraint::Knight { a, b } => {
                f(*a);
                f(*b);
            }
            Constraint::Thermo { cells, len }
            | Constraint::Arrow { cells, len }
            | Constraint::Killer { cells, len, .. } => {
                let len = *len as usize;
                for i in 0..len {
                    f(cells[i]);
                }
            }
        }
    }

    pub fn propagate(&self, state: &mut State) -> Result<bool, Contradiction> {
        match self {
            Constraint::AllDifferent { cells, len } => {
                propagate_all_diff(state, &cells[..*len as usize])
            }
            Constraint::KropkiWhite { a, b } => propagate_kropki_white(state, *a, *b),
            Constraint::KropkiBlack { a, b } => propagate_kropki_black(state, *a, *b),
            Constraint::Thermo { cells, len } => {
                let len = *len as usize;
                propagate_thermo(state, &cells[..len])
            }
            Constraint::Arrow { cells, len } => {
                let len = *len as usize;
                propagate_arrow(state, &cells[..len])
            }
            Constraint::King { a, b }
            | Constraint::Knight { a, b }
            | Constraint::Queen { a, b } => propagate_not_equal(state, *a, *b),
            Constraint::Killer { cells, len, sum } => {
                let len = *len as usize;
                propagate_killer(state, &cells[..len], *sum)
            }
        }
    }
}

fn propagate_all_diff(st: &mut State, cells: &[CellIx]) -> Result<bool, Contradiction> {
    let mut changed = false;
    let complete_set = cells.len() == 9;

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

    if complete_set {
        for d in 1..=9 {
            if count[d as usize] == 1 {
                let bit = 1u16 << d;
                let i = last_pos[d as usize].unwrap();
                if st.assign(i, bit)? {
                    changed = true;
                }
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

fn propagate_thermo(st: &mut State, cells: &[CellIx]) -> Result<bool, Contradiction> {
    let len = cells.len();
    if len == 0 {
        return Ok(false);
    }

    let mut changed = false;

    let mut lower = [1u8; 9];
    let mut upper = [9u8; 9];

    for (idx, &cell) in cells.iter().enumerate() {
        let di = st.domains[cell as usize];
        if di == 0 {
            return Err(Contradiction);
        }

        let mut min_d = 10;
        let mut max_d = 0;
        for d in 1..=9 {
            if di & (1u16 << d) != 0 {
                if d < min_d {
                    min_d = d;
                }
                if d > max_d {
                    max_d = d;
                }
            }
        }

        if min_d == 10 {
            // no digits allowed
            return Err(Contradiction);
        }
        lower[idx] = lower[idx].max(min_d as u8);
        upper[idx] = upper[idx].min(max_d as u8);
    }

    // position i must allow at least i+1,
    // and leave space for remaining cells: <= 9 - (len-1-i)
    for i in 0..len {
        lower[i] = lower[i].max((i + 1) as u8);
        upper[i] = upper[i].min((9 - (len - 1 - i)) as u8);
    }

    // forward pass
    for i in 1..len {
        if lower[i] < lower[i - 1] + 1 {
            lower[i] = lower[i - 1] + 1;
        }
    }

    // backward pass
    for i in (0..len - 1).rev() {
        if upper[i] > upper[i + 1] - 1 {
            upper[i] = upper[i + 1] - 1;
        }
    }

    // check consistency and narrow domain
    for (idx, &cell) in cells.iter().enumerate() {
        let lo = lower[idx];
        let hi = upper[idx];

        if lo > hi {
            return Err(Contradiction);
        }

        let di = st.domains[cell as usize];
        let mut mask: Domain = 0;
        for d in lo..=hi {
            let bit = 1u16 << d;
            if di & bit != 0 {
                mask |= bit;
            }
        }

        if mask == 0 {
            return Err(Contradiction);
        }

        if st.narrow(cell, mask)? {
            changed = true;
        }
    }
    Ok(changed)
}

fn propagate_arrow(st: &mut State, cells: &[CellIx]) -> Result<bool, Contradiction> {
    let len = cells.len();
    if len < 2 {
        return Ok(false);
    }

    let circle = cells[0];
    let arrow_cells = &cells[1..];

    let circle_dom = st.domains[circle as usize];
    if circle_dom == 0 {
        return Err(Contradiction);
    }

    const SUM_LIMIT: usize = 9;

    let mut prefix_sums = [0u16; 9];
    prefix_sums[0] = 1;

    for (i, &cell) in arrow_cells.iter().enumerate() {
        let dom = st.domains[cell as usize];
        if dom == 0 {
            return Err(Contradiction);
        }

        let mut next: u16 = 0;
        for sum in 0..=SUM_LIMIT {
            if prefix_sums[i] & (1 << sum) == 0 {
                continue;
            }
            let mut m = dom;
            while m != 0 {
                let d = m.trailing_zeros() as usize;
                m &= !(1u16 << d);
                let new_sum = sum + d;
                if new_sum <= SUM_LIMIT {
                    next |= 1 << new_sum;
                }
            }
        }
        prefix_sums[i + 1] = next;
    }

    let total_sums = prefix_sums[arrow_cells.len()];
    if total_sums & 0b11_1111_1110 == 0 {
        return Err(Contradiction);
    }

    let mut changed = false;

    let mut circle_mask: Domain = 0;
    for d in 1..=9 {
        if (total_sums & (1 << d)) != 0 && (circle_dom & (1u16 << d)) != 0 {
            circle_mask |= 1u16 << d;
        }
    }

    if circle_mask == 0 {
        return Err(Contradiction);
    }

    if st.narrow(circle, circle_mask)? {
        changed = true;
    }

    let circle_dom = st.domains[circle as usize];

    let mut suffix_sums = [0u16; 9];
    suffix_sums[arrow_cells.len()] = 1;
    for (idx, &cell) in arrow_cells.iter().enumerate().rev() {
        let dom = st.domains[cell as usize];
        if dom == 0 {
            return Err(Contradiction);
        }

        let mut prev: u16 = 0;
        let suf_idx = idx + 1;
        for sum in 0..=SUM_LIMIT {
            if suffix_sums[suf_idx] & (1 << sum) == 0 {
                continue;
            }
            let mut m = dom;
            while m != 0 {
                let d = m.trailing_zeros() as usize;
                m &= !(1u16 << d);
                let new_sum = sum + d;
                if new_sum <= SUM_LIMIT {
                    prev |= 1 << new_sum;
                }
            }
        }
        suffix_sums[idx] = prev;
    }

    for (j, &cell) in arrow_cells.iter().enumerate() {
        let dom = st.domains[cell as usize];
        if dom == 0 {
            return Err(Contradiction);
        }

        let mut others_sums: u16 = 0;
        for a in 0..=SUM_LIMIT {
            if prefix_sums[j] & (1 << a) == 0 {
                continue;
            }
            for b in 0..=SUM_LIMIT {
                if suffix_sums[j + 1] & (1 << b) == 0 {
                    continue;
                }
                let sum = a + b;
                if sum <= SUM_LIMIT {
                    others_sums |= 1 << sum;
                }
            }
        }

        let mut new_mask: Domain = 0;
        let mut m = dom;
        while m != 0 {
            let d = m.trailing_zeros() as u8;
            m &= !(1u16 << d);

            let mut cm = circle_dom;
            let mut ok = false;
            while cm != 0 {
                let c = cm.trailing_zeros() as usize;
                cm &= !(1u16 << c);
                if c >= d as usize && (others_sums & (1 << (c - d as usize))) != 0 {
                    ok = true;
                    break;
                }
            }

            if ok {
                new_mask |= 1u16 << d;
            }
        }

        if new_mask == 0 {
            return Err(Contradiction);
        }

        if st.narrow(cell, new_mask)? {
            changed = true;
        }
    }

    Ok(changed)
}

fn propagate_not_equal(st: &mut State, a: CellIx, b: CellIx) -> Result<bool, Contradiction> {
    let da = st.domains[a as usize];
    let db = st.domains[b as usize];

    if da == 0 || db == 0 {
        return Err(Contradiction);
    }

    let mut changed = false;

    if da.count_ones() == 1 && db.count_ones() == 1 && da == db {
        return Err(Contradiction);
    }

    if da.count_ones() == 1 {
        let forbidden = da;
        let new_db = db & !forbidden;
        if new_db == 0 {
            return Err(Contradiction);
        }
        if st.narrow(b, new_db)? {
            changed = true;
        }
    }

    if db.count_ones() == 1 {
        let forbidden = db;
        let new_da = da & !forbidden;
        if new_da == 0 {
            return Err(Contradiction);
        }
        if st.narrow(a, new_da)? {
            changed = true;
        }
    }

    Ok(changed)
}

fn propagate_killer(st: &mut State, cells: &[CellIx], sum: u8) -> Result<bool, Contradiction> {
    let len = cells.len();
    if len == 0 {
        return Ok(false);
    }

    let mut changed = false;

    let mut mins = [0u8; 9];
    let mut maxs = [0u8; 9];

    for (i, &cell) in cells.iter().enumerate() {
        let dom = st.domains[cell as usize];
        if dom == 0 {
            return Err(Contradiction);
        }

        let mut min_d = 10;
        let mut max_d = 0;

        let mut m = dom;
        while m != 0 {
            let d = m.trailing_zeros() as u8;
            m &= !(1u16 << d);
            if d < min_d {
                min_d = d;
            }
            if d > max_d {
                max_d = d;
            }
        }

        if min_d == 10 {
            return Err(Contradiction);
        }

        mins[i] = min_d;
        maxs[i] = max_d;
    }

    let mut total_min: u8 = 0;
    let mut total_max: u8 = 0;
    for i in 0..len {
        total_min = total_min.saturating_add(mins[i]);
        total_max = total_max.saturating_add(maxs[i]);
    }

    if sum < total_min || sum > total_max {
        return Err(Contradiction);
    }

    for (idx, &cell) in cells.iter().enumerate() {
        let min_i = mins[idx];
        let max_i = maxs[idx];

        let others_min = total_min.saturating_sub(min_i);
        let others_max = total_max.saturating_sub(max_i);

        let lo_allowed = (sum.saturating_sub(others_max)).max(min_i);
        let hi_allowed = (sum.saturating_sub(others_min)).min(max_i);

        if lo_allowed > hi_allowed {
            return Err(Contradiction);
        }

        let dom = st.domains[cell as usize];
        let mut mask: Domain = 0;
        for d in lo_allowed..=hi_allowed {
            let bit = 1u16 << d;
            if dom & bit != 0 {
                mask |= bit;
            }
        }

        if mask == 0 {
            return Err(Contradiction);
        }

        if st.narrow(cell, mask)? {
            changed = true;
        }
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
    fn all_diff_respects_length_for_shorter_scopes() {
        let mut st = State::new();

        let cells: [CellIx; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

        // cell 0 is 1; cell 1 cannot keep 1 after propagation on first two cells
        st.domains[0] = mask(&[1]);
        st.domains[1] = mask(&[1, 2, 3]);

        let changed = propagate_all_diff(&mut st, &cells[..2]).unwrap();
        assert!(changed);
        assert_eq!(st.domains[1], mask(&[2, 3]));

        // untouched cells beyond len keep their full domain
        for i in 2..9 {
            assert_eq!(
                st.domains[i], DIGITS_MASK,
                "cell {} unexpectedly changed",
                i
            );
        }
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
        st.domains[b as usize] = mask(&[3]);

        let changed = propagate_kropki_black(&mut st, a, b).unwrap();
        assert!(changed);

        assert_eq!(st.domains[a as usize], mask(&[6]));
        assert_eq!(st.domains[b as usize], mask(&[3]));
    }

    #[test]
    fn kropki_black_symmetric_narrowing() {
        let mut st = State::new();
        let a: CellIx = 0;
        let b: CellIx = 1;

        // a can be {1,2,3,4}, b can be {2,3,4,5}
        st.domains[a as usize] = mask(&[1, 2, 3, 4]);
        st.domains[b as usize] = mask(&[2, 3, 4, 5]);

        let changed = propagate_kropki_black(&mut st, a, b).unwrap();
        assert!(changed);

        // Valid ratio-2 pairs in 1..9: (1,2), (2,4), (3,6), (4,8)
        // Intersect with our sets:
        //   a ∈ {1,2,3,4}, b ∈ {2,3,4,5}
        // Possible pairs: (1,2), (2,4)
        // So a ∈ {1,2,4} , b ∈ {2,4}
        assert_eq!(st.domains[a as usize], mask(&[1, 2, 4]));
        assert_eq!(st.domains[b as usize], mask(&[2, 4]));
    }

    #[test]
    fn kropki_black_detects_contradiction_when_no_ratio_in_two_pairs() {
        let mut st = State::new();
        let a: CellIx = 0;
        let b: CellIx = 1;

        // a is fixed to 1, b is fixed to 4 -> no ratio of two
        st.domains[a as usize] = mask(&[1]);
        st.domains[b as usize] = mask(&[4]);

        let res = propagate_kropki_black(&mut st, a, b);
        assert!(res.is_err());
    }
}
