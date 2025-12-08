use crate::{
    Engine, NN, add_all_sudoku_constraints, add_arrow, add_killer_cage, add_king_constraints,
    add_knight_constraints, add_kropki_black, add_kropki_white, add_queen_constraints, add_thermo,
    engine::EngineRng,
    types::{digit_of_bit, idx},
};

use std::{
    collections::HashSet,
    ops::RangeInclusive,
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

    pub fn seed(&self) -> u64 {
        self.0
    }
}

impl EngineRng for SimpleRng {
    fn gen_range(&mut self, range: std::ops::Range<usize>) -> usize {
        SimpleRng::gen_range(self, range)
    }
}

fn shuffle<R: EngineRng, T>(rng: &mut R, slice: &mut [T]) {
    let len = slice.len();
    if len <= 1 {
        return;
    }

    for i in (1..len).rev() {
        let j = rng.gen_range(0..i + 1); // random in [0, i]
        slice.swap(i, j);
    }
}

fn symmetric_of(pos: usize, symmetry: Symmetry) -> usize {
    let r = pos / 9;
    let c = pos % 9;
    match symmetry {
        Symmetry::Rotational180 => (8 - r) * 9 + (8 - c),
        Symmetry::MirrorH => (8 - r) * 9 + c,
        Symmetry::MirrorV => r * 9 + (8 - c),
        Symmetry::DiagMain => c * 9 + r,
        Symmetry::DiagAnti => (8 - c) * 9 + (8 - r),
    }
}

fn clue_count(puzzle: &[Option<u8>]) -> usize {
    puzzle.iter().filter(|c| c.is_some()).count()
}

fn apply_specs(engine: &mut Engine, specs: &[VariantSpec]) {
    for spec in specs {
        match spec {
            VariantSpec::KropkiWhite(a, b) => add_kropki_white(engine, *a, *b),
            VariantSpec::KropkiBlack(a, b) => add_kropki_black(engine, *a, *b),
            VariantSpec::Thermo(path) => add_thermo(engine, path),
            VariantSpec::Arrow(path) => add_arrow(engine, path),
            VariantSpec::Killer {
                cells,
                sum,
                no_repeats,
            } => add_killer_cage(engine, cells, *sum, *no_repeats),
            VariantSpec::King => add_king_constraints(engine),
            VariantSpec::Knight => add_knight_constraints(engine),
            VariantSpec::Queen => add_queen_constraints(engine),
        }
    }
}

fn fill_killer_sums(solution: &[u8; NN], specs: &mut [VariantSpec]) {
    for spec in specs.iter_mut() {
        if let VariantSpec::Killer {
            cells,
            sum,
            no_repeats: _,
        } = spec
        {
            let mut s: u8 = 0;
            for &(r, c) in cells.iter() {
                let i = r * 9 + c;
                s = s.saturating_add(solution[i] as u8);
            }
            *sum = s;
        }
    }
}

fn apply_specs_without_killer_sums(engine: &mut Engine, specs: &[VariantSpec]) {
    for spec in specs {
        match spec {
            VariantSpec::Killer {
                cells,
                no_repeats: true,
                ..
            } => {
                let mut arr = [0u8; 9];
                for (i, &(r, c)) in cells.iter().enumerate() {
                    arr[i] = idx(r, c);
                }
                engine.add_constraint(crate::Constraint::AllDifferent {
                    cells: arr,
                    len: cells.len() as u8,
                });
            }
            VariantSpec::Killer { .. } => {}
            VariantSpec::KropkiWhite(a, b) => add_kropki_white(engine, *a, *b),
            VariantSpec::KropkiBlack(a, b) => add_kropki_black(engine, *a, *b),
            VariantSpec::Thermo(path) => add_thermo(engine, path),
            VariantSpec::Arrow(path) => add_arrow(engine, path),
            VariantSpec::King => add_king_constraints(engine),
            VariantSpec::Knight => add_knight_constraints(engine),
            VariantSpec::Queen => add_queen_constraints(engine),
        }
    }
}

fn weighted_choice(rng: &mut SimpleRng, entries: &[VariantPoolEntry]) -> Option<VariantKind> {
    let total: u64 = entries.iter().map(|e| e.weight as u64).sum();
    if total == 0 {
        return None;
    }
    let pick = rng.next_u32() as u64 % total;
    let mut acc = 0u64;
    for e in entries {
        acc += e.weight as u64;
        if pick < acc {
            return Some(e.kind);
        }
    }
    None
}

fn choose_variant_kinds(cfg: &GenerationConfig, rng: &mut SimpleRng) -> Vec<VariantKind> {
    let mut out = cfg.required_variants.clone();
    let target = {
        let (start, end) = (*cfg.variant_count.start(), *cfg.variant_count.end());
        if start == end {
            start
        } else {
            rng.gen_range(start..end + 1)
        }
    };

    let mut guard = 0;
    while out.len() < target && guard < 50 {
        guard += 1;
        if let Some(kind) = weighted_choice(rng, &cfg.variant_pool) {
            if matches!(
                kind,
                VariantKind::King | VariantKind::Knight | VariantKind::Queen
            ) {
                if out.contains(&kind) {
                    continue;
                }
            }
            out.push(kind);
        }
    }
    out
}

fn gen_kropki_specs(
    rng: &mut SimpleRng,
    whites: usize,
    blacks: usize,
    used_edges: &mut HashSet<((usize, usize), (usize, usize))>,
) -> Vec<VariantSpec> {
    let mut edges = Vec::with_capacity(144);
    for r in 0..9 {
        for c in 0..9 {
            if c + 1 < 9 {
                edges.push(((r, c), (r, c + 1)));
            }
            if r + 1 < 9 {
                edges.push(((r, c), (r + 1, c)));
            }
        }
    }
    shuffle(rng, &mut edges);

    let mut out = Vec::new();
    for edge in edges.iter() {
        let mut norm = *edge;
        if norm.1 < norm.0 {
            norm = (norm.1, norm.0);
        }
        if used_edges.contains(&norm) {
            continue;
        }
        if out
            .iter()
            .filter(|v| matches!(v, VariantSpec::KropkiWhite(..)))
            .count()
            < whites
        {
            used_edges.insert(norm);
            out.push(VariantSpec::KropkiWhite(edge.0, edge.1));
        } else if out
            .iter()
            .filter(|v| matches!(v, VariantSpec::KropkiBlack(..)))
            .count()
            < blacks
        {
            used_edges.insert(norm);
            out.push(VariantSpec::KropkiBlack(edge.0, edge.1));
        }
        if out.len() >= whites + blacks {
            break;
        }
    }
    out
}

fn random_path(
    rng: &mut SimpleRng,
    length: RangeInclusive<usize>,
    occupied: &mut HashSet<(usize, usize)>,
) -> Option<Vec<(usize, usize)>> {
    let min_len = *length.start();
    let max_len = *length.end();
    let mut attempts = 0;
    while attempts < 40 {
        attempts += 1;
        let target_len = if min_len == max_len {
            min_len
        } else {
            rng.gen_range(min_len..max_len + 1)
        };
        let start_r = rng.gen_range(0..9);
        let start_c = rng.gen_range(0..9);
        if occupied.contains(&(start_r, start_c)) {
            continue;
        }
        let mut path = vec![(start_r, start_c)];
        let mut used_local: HashSet<(usize, usize)> = HashSet::new();
        used_local.insert((start_r, start_c));
        while path.len() < target_len {
            let (r, c) = path[path.len() - 1];
            let mut options = Vec::with_capacity(4);
            if r > 0 {
                options.push((r - 1, c));
            }
            if r + 1 < 9 {
                options.push((r + 1, c));
            }
            if c > 0 {
                options.push((r, c - 1));
            }
            if c + 1 < 9 {
                options.push((r, c + 1));
            }
            shuffle(rng, &mut options);
            let mut placed = false;
            for (nr, nc) in options {
                if used_local.contains(&(nr, nc)) || occupied.contains(&(nr, nc)) {
                    continue;
                }
                path.push((nr, nc));
                used_local.insert((nr, nc));
                placed = true;
                break;
            }
            if !placed {
                break;
            }
        }
        if path.len() >= min_len {
            for cell in path.iter() {
                occupied.insert(*cell);
            }
            return Some(path);
        }
    }
    None
}

fn gen_thermo_specs(
    rng: &mut SimpleRng,
    count: usize,
    length: RangeInclusive<usize>,
    occupied: &mut HashSet<(usize, usize)>,
) -> Vec<VariantSpec> {
    let mut out = Vec::new();
    for _ in 0..count {
        if let Some(path) = random_path(rng, length.clone(), occupied) {
            out.push(VariantSpec::Thermo(path));
        }
    }
    out
}

fn gen_arrow_specs(
    rng: &mut SimpleRng,
    count: usize,
    length: RangeInclusive<usize>,
    occupied: &mut HashSet<(usize, usize)>,
) -> Vec<VariantSpec> {
    let mut out = Vec::new();
    for _ in 0..count {
        if let Some(path) = random_path(rng, length.clone(), occupied) {
            if path.len() < 2 {
                continue;
            }
            // treat first as circle
            out.push(VariantSpec::Arrow(path));
        }
    }
    out
}

fn random_connected_cells(
    rng: &mut SimpleRng,
    len: usize,
    occupied: &HashSet<(usize, usize)>,
) -> Option<Vec<(usize, usize)>> {
    if len == 0 {
        return None;
    }
    let mut candidates: Vec<(usize, usize)> = (0..9)
        .flat_map(|r| (0..9).map(move |c| (r, c)))
        .filter(|cell| !occupied.contains(cell))
        .collect();
    shuffle(rng, &mut candidates);

    let neighbor_offsets = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)];

    for start in candidates.into_iter().take(40) {
        let mut cage = vec![start];
        let mut used = HashSet::new();
        used.insert(start);
        while cage.len() < len {
            let mut neighbors = Vec::new();
            let (cr, cc) = *cage.last().unwrap();
            for (dr, dc) in neighbor_offsets {
                let nr = cr as i32 + dr;
                let nc = cc as i32 + dc;
                if nr < 0 || nr >= 9 || nc < 0 || nc >= 9 {
                    continue;
                }
                let cell = (nr as usize, nc as usize);
                if occupied.contains(&cell) || used.contains(&cell) {
                    continue;
                }
                neighbors.push(cell);
            }
            if neighbors.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..neighbors.len());
            let next = neighbors[idx];
            cage.push(next);
            used.insert(next);
        }
        if cage.len() == len {
            return Some(cage);
        }
    }
    None
}

fn gen_killer_specs(
    rng: &mut SimpleRng,
    count: usize,
    size: RangeInclusive<usize>,
    no_repeats: bool,
    occupied: &mut HashSet<(usize, usize)>,
) -> Vec<VariantSpec> {
    let mut out = Vec::new();

    let min_size = *size.start();
    let max_size = *size.end();

    for _ in 0..count {
        let len = if min_size == max_size {
            min_size
        } else {
            rng.gen_range(min_size..max_size + 1)
        };
        if len == 0 {
            continue;
        }

        if let Some(cells) = random_connected_cells(rng, len, occupied) {
            for cell in cells.iter() {
                occupied.insert(*cell);
            }
            out.push(VariantSpec::Killer {
                cells,
                sum: 0,
                no_repeats,
            });
        }
    }
    out
}

fn sample_range(rng: &mut SimpleRng, range: &RangeInclusive<usize>) -> usize {
    let (start, end) = (*range.start(), *range.end());
    if start == end {
        start
    } else {
        rng.gen_range(start..end + 1)
    }
}

fn instantiate_variants(cfg: &GenerationConfig, rng: &mut SimpleRng) -> Vec<VariantSpec> {
    let kinds = choose_variant_kinds(cfg, rng);
    let mut specs = Vec::new();
    let mut used_edges: HashSet<((usize, usize), (usize, usize))> = HashSet::new();
    let mut occupied: HashSet<(usize, usize)> = HashSet::new();

    for kind in kinds {
        match kind {
            VariantKind::Kropki => {
                let whites = sample_range(rng, &cfg.kropki_white);
                let blacks = sample_range(rng, &cfg.kropki_black);
                specs.extend(gen_kropki_specs(rng, whites, blacks, &mut used_edges));
            }
            VariantKind::Thermo => {
                let count = sample_range(rng, &cfg.thermo_count);
                specs.extend(gen_thermo_specs(
                    rng,
                    count,
                    cfg.thermo_length.clone(),
                    &mut occupied,
                ));
            }
            VariantKind::Arrow => {
                let count = sample_range(rng, &cfg.arrow_count);
                specs.extend(gen_arrow_specs(
                    rng,
                    count,
                    cfg.arrow_length.clone(),
                    &mut occupied,
                ));
            }
            VariantKind::Killer => {
                let count = sample_range(rng, &cfg.killer_count);
                specs.extend(gen_killer_specs(
                    rng,
                    count,
                    cfg.killer_size.clone(),
                    cfg.killer_no_repeats,
                    &mut occupied,
                ));
            }
            VariantKind::King => specs.push(VariantSpec::King),
            VariantKind::Knight => specs.push(VariantSpec::Knight),
            VariantKind::Queen => specs.push(VariantSpec::Queen),
        }
    }

    specs
}

fn removal_units(symmetry: Option<Symmetry>, rng: &mut SimpleRng) -> Vec<Vec<usize>> {
    match symmetry {
        None => {
            let mut positions: Vec<usize> = (0..NN).collect();
            shuffle(rng, &mut positions);
            positions.into_iter().map(|p| vec![p]).collect()
        }
        Some(sym) => {
            let mut visited = [false; NN];
            let mut units = Vec::new();
            for pos in 0..NN {
                if visited[pos] {
                    continue;
                }
                let partner = symmetric_of(pos, sym);
                visited[pos] = true;
                visited[partner] = true;
                if partner == pos {
                    units.push(vec![pos]);
                } else {
                    units.push(vec![pos, partner]);
                }
            }
            shuffle(rng, &mut units);
            units
        }
    }
}

fn constraint_cells(specs: &[VariantSpec]) -> HashSet<usize> {
    let mut out = HashSet::new();
    for spec in specs {
        match spec {
            VariantSpec::KropkiWhite((r1, c1), (r2, c2))
            | VariantSpec::KropkiBlack((r1, c1), (r2, c2)) => {
                out.insert(r1 * 9 + c1);
                out.insert(r2 * 9 + c2);
            }
            VariantSpec::Thermo(path) | VariantSpec::Arrow(path) => {
                for &(r, c) in path {
                    out.insert(r * 9 + c);
                }
            }
            VariantSpec::Killer { cells, .. } => {
                for &(r, c) in cells {
                    out.insert(r * 9 + c);
                }
            }
            VariantSpec::King | VariantSpec::Knight | VariantSpec::Queen => {}
        }
    }
    out
}

fn prioritize_units(
    units: &mut Vec<Vec<usize>>,
    constraint_cells: &HashSet<usize>,
    rng: &mut SimpleRng,
) {
    shuffle(rng, units);
    units.sort_by(|a, b| {
        let ca = a.iter().filter(|p| constraint_cells.contains(p)).count();
        let cb = b.iter().filter(|p| constraint_cells.contains(p)).count();
        cb.cmp(&ca)
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symmetry {
    Rotational180,
    MirrorH,
    MirrorV,
    DiagMain,
    DiagAnti,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Minimality {
    None,
    Strict,
    AtMost(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariantKind {
    Kropki,
    Thermo,
    Arrow,
    Killer,
    King,
    Knight,
    Queen,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariantSpec {
    KropkiWhite((usize, usize), (usize, usize)),
    KropkiBlack((usize, usize), (usize, usize)),
    Thermo(Vec<(usize, usize)>),
    Arrow(Vec<(usize, usize)>),
    Killer {
        cells: Vec<(usize, usize)>,
        sum: u8,
        no_repeats: bool,
    },
    King,
    Knight,
    Queen,
}

#[derive(Clone, Debug)]
pub struct VariantPoolEntry {
    pub kind: VariantKind,
    pub weight: u32,
}

#[derive(Clone, Debug)]
pub struct GenerationConfig {
    pub seed: Option<u64>,
    pub required_variants: Vec<VariantKind>,
    pub variant_pool: Vec<VariantPoolEntry>,
    pub variant_count: RangeInclusive<usize>,
    pub symmetry: Option<Symmetry>,
    pub clue_target: Option<usize>,
    pub clue_range: Option<RangeInclusive<usize>>,
    pub minimality: Minimality,
    pub kropki_white: RangeInclusive<usize>,
    pub kropki_black: RangeInclusive<usize>,
    pub thermo_count: RangeInclusive<usize>,
    pub thermo_length: RangeInclusive<usize>,
    pub arrow_count: RangeInclusive<usize>,
    pub arrow_length: RangeInclusive<usize>,
    pub killer_count: RangeInclusive<usize>,
    pub killer_size: RangeInclusive<usize>,
    pub killer_no_repeats: bool,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            seed: None,
            required_variants: Vec::new(),
            variant_pool: vec![
                VariantPoolEntry {
                    kind: VariantKind::Kropki,
                    weight: 1,
                },
                VariantPoolEntry {
                    kind: VariantKind::Thermo,
                    weight: 1,
                },
                VariantPoolEntry {
                    kind: VariantKind::Arrow,
                    weight: 1,
                },
                VariantPoolEntry {
                    kind: VariantKind::Killer,
                    weight: 1,
                },
                VariantPoolEntry {
                    kind: VariantKind::King,
                    weight: 1,
                },
                VariantPoolEntry {
                    kind: VariantKind::Knight,
                    weight: 1,
                },
                //VariantPoolEntry {
                //    kind: VariantKind::Queen,
                //    weight: 1,
                //},
            ],
            variant_count: 1..=3,
            symmetry: None,
            clue_target: Some(30),
            clue_range: None,
            minimality: Minimality::None,
            kropki_white: 6..=12,
            kropki_black: 6..=12,
            thermo_count: 2..=4,
            thermo_length: 3..=6,
            arrow_count: 2..=4,
            arrow_length: 3..=4,
            killer_count: 2..=4,
            killer_size: 2..=4,
            killer_no_repeats: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedPuzzle {
    pub puzzle: String,
    pub solution: [u8; NN],
    pub constraints: Vec<VariantSpec>,
    pub seed: u64,
    pub engine: Engine,
    pub clue_count: usize,
    pub symmetry: Option<Symmetry>,
}

pub fn generate_full_solution(rng: SimpleRng) -> [u8; NN] {
    let mut rng = rng;
    generate_full_solution_with_rng(&mut rng, |_| {})
}

pub fn generate_full_solution_with<F>(rng: SimpleRng, extra: F) -> [u8; NN]
where
    F: FnOnce(&mut Engine),
{
    let mut rng = rng;
    generate_full_solution_with_rng(&mut rng, extra)
}

pub fn generate_full_solution_with_rng<F, R>(rng: &mut R, extra: F) -> [u8; NN]
where
    F: FnOnce(&mut Engine),
    R: EngineRng,
{
    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);
    extra(&mut eng);

    eng.search_with_rng(rng).expect("search failed");
    assert!(eng.solved());

    let mut out = [0u8; NN];

    for i in 0..NN {
        let dom = eng.state.domains[i];
        out[i] = digit_of_bit(dom).unwrap();
    }

    if eng.constraints.is_empty() {
        let mut digits = [1u8, 2, 3, 4, 5, 6, 7, 8, 9];
        shuffle(rng, &mut digits);

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
    let sol = generate_full_solution_with_rng(&mut rng, extra);
    let mut puzzle: Vec<Option<u8>> = sol.iter().copied().map(Some).collect();

    // random order of position try to remove
    let mut positions: Vec<usize> = (0..NN).collect();
    shuffle(&mut rng, &mut positions);

    // try to remove clues while preserving uniqueness
    for pos in positions {
        let saved = puzzle[pos];
        puzzle[pos] = None;

        let puzzle_str = puzzle_vec_to_string(&puzzle);
        if !has_unique_solution_from_string_with(&puzzle_str, extra, &mut rng) {
            puzzle[pos] = saved;
        }
        let clues_now = puzzle.iter().filter(|c| c.is_some()).count();
        if clues_now <= target_clues {
            break;
        }
    }
    puzzle_vec_to_string(&puzzle)
}

pub fn generate_random_variant_puzzle(cfg: GenerationConfig) -> GeneratedPuzzle {
    let mut rng = match cfg.seed {
        Some(seed) => SimpleRng::from_seed(seed),
        None => SimpleRng::new(),
    };
    let seed = rng.seed();

    let mut specs = instantiate_variants(&cfg, &mut rng);
    println!("specs are: {:?}", specs);
    let spec_clone = specs.clone();
    let solution = generate_full_solution_with_rng(&mut rng, |eng| {
        apply_specs_without_killer_sums(eng, &spec_clone);
    });

    fill_killer_sums(&solution, &mut specs);

    let mut puzzle: Vec<Option<u8>> = solution.iter().copied().map(Some).collect();

    let clue_range = cfg.clue_range.clone();
    let clue_target = cfg.clue_target;

    let desired_min = clue_range
        .as_ref()
        .map(|r| *r.start())
        .or(clue_target)
        .unwrap_or(0);
    let desired_max = clue_range
        .as_ref()
        .map(|r| *r.end())
        .or(clue_target)
        .unwrap_or(desired_min);

    let constraint_cells = constraint_cells(&specs);
    let mut units = removal_units(cfg.symmetry, &mut rng);
    prioritize_units(&mut units, &constraint_cells, &mut rng);
    let stop_threshold = match cfg.minimality {
        Minimality::None => Some(desired_max),
        Minimality::AtMost(k) => Some(desired_max + k),
        Minimality::Strict => None,
    };

    'outer: loop {
        let mut changed = false;
        for unit in units.iter() {
            if unit.iter().all(|&p| puzzle[p].is_none()) {
                continue;
            }
            let saved: Vec<(usize, Option<u8>)> = unit.iter().map(|&p| (p, puzzle[p])).collect();
            for &p in unit.iter() {
                puzzle[p] = None;
            }
            let clues_now = clue_count(&puzzle);
            if clues_now < desired_min {
                for (p, v) in saved {
                    puzzle[p] = v;
                }
                continue;
            }
            let puzzle_str = puzzle_vec_to_string(&puzzle);
            if !has_unique_solution_with_specs(&puzzle_str, &specs, &mut rng) {
                for (p, v) in saved {
                    puzzle[p] = v;
                }
                continue;
            }
            changed = true;
            if let Some(stop) = stop_threshold {
                if clues_now <= stop {
                    break 'outer;
                }
            }
        }

        if !matches!(cfg.minimality, Minimality::Strict) || !changed {
            break;
        }
        prioritize_units(&mut units, &constraint_cells, &mut rng);
    }

    let puzzle_str = puzzle_vec_to_string(&puzzle);

    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);
    apply_specs(&mut eng, &specs);
    eng.load_givens(&puzzle_str).unwrap();
    eng.search().unwrap();
    assert!(eng.solved());

    GeneratedPuzzle {
        puzzle: puzzle_str,
        solution,
        constraints: specs,
        seed,
        engine: eng,
        clue_count: clue_count(&puzzle),
        symmetry: cfg.symmetry,
    }
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

fn has_unique_solution_from_string_with<F, R>(puzzle: &str, extra: F, rng: &mut R) -> bool
where
    F: Fn(&mut Engine),
    R: EngineRng,
{
    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);
    extra(&mut eng);

    if eng.load_givens(puzzle).is_err() {
        return false;
    }

    eng.has_unique_solution_with_rng(rng)
}

fn has_unique_solution_with_specs<R: EngineRng>(puzzle: &str, specs: &[VariantSpec], rng: &mut R) -> bool {
    let mut eng = Engine::new();
    add_all_sudoku_constraints(&mut eng);
    apply_specs(&mut eng, specs);

    if eng.load_givens(puzzle).is_err() {
        return false;
    }

    eng.has_unique_solution_with_rng(rng)
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

    #[test]
    fn generate_random_variant_puzzle_is_unique_and_reports_branches() {
        let cfg = GenerationConfig {
            seed: Some(4242),
            clue_target: Some(32),
            minimality: Minimality::AtMost(0),
            ..Default::default()
        };
        let out = generate_random_variant_puzzle(cfg);
        assert_eq!(out.clue_count, clue_count(&out.puzzle));
        let mut rng = SimpleRng::from_seed(out.seed);
        assert!(has_unique_solution_with_specs(
            &out.puzzle,
            &out.constraints,
            &mut rng
        ));
    }

    #[test]
    fn symmetry_is_respected_when_requested() {
        let cfg = GenerationConfig {
            seed: Some(999),
            variant_count: 0..=0,
            symmetry: Some(Symmetry::Rotational180),
            clue_target: Some(34),
            minimality: Minimality::AtMost(0),
            ..Default::default()
        };
        let out = generate_random_variant_puzzle(cfg);
        let bytes: Vec<char> = out.puzzle.chars().collect();
        for i in 0..NN {
            let j = symmetric_of(i, Symmetry::Rotational180);
            let is_clue_i = bytes[i].is_ascii_digit() && bytes[i] != '.';
            let is_clue_j = bytes[j].is_ascii_digit() && bytes[j] != '.';
            assert_eq!(is_clue_i, is_clue_j);
        }
    }

    #[test]
    fn killer_cages_are_orthogonally_connected() {
        let cfg = GenerationConfig {
            seed: Some(17),
            required_variants: vec![VariantKind::Killer],
            variant_pool: Vec::new(),
            variant_count: 1..=1,
            killer_count: 1..=2,
            killer_size: 3..=4,
            clue_target: Some(40),
            minimality: Minimality::AtMost(0),
            ..Default::default()
        };
        let out = generate_random_variant_puzzle(cfg);
        for spec in out.constraints {
            if let VariantSpec::Killer { cells, .. } = spec {
                for &(r, c) in cells.iter() {
                    let mut has_neighbor = false;
                    for (dr, dc) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                        let nr = r as i32 + dr;
                        let nc = c as i32 + dc;
                        if cells
                            .iter()
                            .any(|&(rr, cc)| rr as i32 == nr && cc as i32 == nc)
                        {
                            has_neighbor = true;
                            break;
                        }
                    }
                    assert!(has_neighbor || cells.len() == 1);
                }
            }
        }
    }

    #[test]
    fn constraint_cells_prioritized_in_removal_units() {
        let mut units = vec![vec![0], vec![40], vec![10]];
        let mut constraint_cells = std::collections::HashSet::new();
        constraint_cells.insert(10usize);
        constraint_cells.insert(40usize);
        let mut rng = SimpleRng::from_seed(123);
        prioritize_units(&mut units, &constraint_cells, &mut rng);
        let first_score = units[0]
            .iter()
            .filter(|p| constraint_cells.contains(p))
            .count();
        let last_score = units
            .last()
            .unwrap()
            .iter()
            .filter(|p| constraint_cells.contains(p))
            .count();
        assert!(first_score >= last_score);
    }
}
