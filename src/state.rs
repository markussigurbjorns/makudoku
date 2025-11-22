use std::collections::VecDeque;

use crate::{CellIx, Contradiction, DIGITS_MASK, Domain, NN};

#[derive(Clone)]
pub struct State {
    pub domains: [Domain; NN],
    pub(crate) trail: Vec<(CellIx, Domain)>,
    pub(crate) queue: VecDeque<usize>,
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

    pub fn print_domain(&self) {
        for bit in self.domains {
            println!("{:09b}", bit >> 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::bit_of_digit;

    fn mask(digits: &[u8]) -> Domain {
        digits.iter().fold(0, |acc, &d| acc | bit_of_digit(d))
    }

    #[test]
    fn new_initializes_domains_trail_and_queue() {
        let st = State::new();

        // all domains should be full DIGITS_MASK
        for (i, d) in st.domains.iter().enumerate() {
            assert_eq!(
                *d, DIGITS_MASK,
                "domain at index {} not initialized to DIGITS_MASK",
                i
            );
        }

        assert!(st.trail.is_empty());
        assert!(st.queue.is_empty());
    }

    #[test]
    fn narrow_reduced_domain_and_records_trail() {
        let mut st = State::new();
        let i: CellIx = 5;

        // start with full domain, narrow to {3,4}
        let old = st.domains[i as usize];
        let new_mask = mask(&[3, 4]);

        let changed = st.narrow(i, new_mask).unwrap();
        assert!(changed);

        assert_eq!(st.domains[i as usize], new_mask);
        assert_eq!(st.trail.len(), 1);
        assert_eq!(st.trail[0], (i, old));
    }

    #[test]
    fn narrow_no_change_when_mask_is_superset() {
        let mut st = State::new();
        let i: CellIx = 7;

        // domain is {3,4,5}
        let domain = mask(&[3, 4, 5]);
        st.domains[i as usize] = domain;

        // mask still allows all of them -> no change
        let mask_superset = mask(&[1, 2, 3, 4, 5]);
        let changed = st.narrow(i, mask_superset).unwrap();

        assert!(!changed);
        assert_eq!(st.domains[i as usize], domain);
        assert!(st.trail.is_empty());
    }

    #[test]
    fn narrow_contradiction_when_mask_wipes_out_domain() {
        let mut st = State::new();
        let i: CellIx = 3;

        st.domains[i as usize] = mask(&[1, 2, 3]);

        // mask excludes 1,2,3 -> new = 0 -> Contradiction
        let mask_zero = mask(&[4, 5, 6]);
        let res = st.narrow(i, mask_zero);

        assert!(res.is_err());
        assert_eq!(st.domains[i as usize], mask(&[1, 2, 3]));
        assert!(st.trail.is_empty());
    }

    #[test]
    fn assign_behaves_like_narrow_for_single_bit() {
        let mut st = State::new();
        let i: CellIx = 10;

        let single = bit_of_digit(7);

        let changed = st.assign(i, single).unwrap();

        assert!(changed);
        assert_eq!(st.domains[i as usize], single);
        assert_eq!(st.trail.len(), 1);
        assert_eq!(st.trail[0], (i, DIGITS_MASK));
    }

    #[test]
    fn backtrack_restores_previous_domains() {
        let mut st = State::new();
        let i: CellIx = 0;
        let j: CellIx = 1;

        // first change: narrow cell i
        let trail_len0 = st.trail.len();
        st.narrow(i, mask(&[1, 2, 3])).unwrap();
        let trail_len1 = st.trail.len();
        assert_eq!(trail_len1, trail_len0 + 1);

        // second change: narrow cell j
        st.narrow(j, mask(&[4, 5, 6])).unwrap();
        let trail_len2 = st.trail.len();
        assert_eq!(trail_len2, trail_len1 + 1);

        // backtrack to trail_len1
        // gets rid of change on cell j
        st.backtrack_to(trail_len1);
        assert_eq!(st.trail.len(), trail_len1);

        // backtrack all the way
        st.backtrack_to(trail_len0);
        assert_eq!(st.trail.len(), trail_len0);
        assert_eq!(st.domains[i as usize], DIGITS_MASK);
    }

    #[test]
    fn backtrack_to_noop_when_trail_already_shorter() {
        let mut st = State::new();
        let i: CellIx = 0;

        st.narrow(i, mask(&[1, 2, 3])).unwrap();
        let trail_len = st.trail.len();
        assert_eq!(trail_len, 1);

        // backtrack to a larger number than current length should do nothing
        st.backtrack_to(10);
        assert_eq!(st.trail.len(), 1);
    }
}
