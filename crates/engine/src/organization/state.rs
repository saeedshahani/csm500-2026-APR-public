use super::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Opcode {
    Equal,
    Difference,
    Accept,
}
#[derive(Clone, Debug)]
pub(super) struct Operator {
    pub word: usize,
    pub input: usize,
    pub output: usize,
    pub recognizes: Vec<usize>,
}
#[derive(Clone, Debug)]
pub(super) struct Program {
    pub id: u64,
    pub role: usize,
    pub code: Opcode,
    pub created: u64,
    pub operator: usize,
}
#[derive(Clone, Debug)]
pub(super) struct Model {
    pub from: usize,
    pub action: usize,
    pub target: usize,
    pub witness_epoch: u64,
    pub witness_clock: Vec<u64>,
    pub last_checked: i64,
    pub revision: u64,
}
#[derive(Clone, Debug)]
pub(super) struct Witness {
    pub from: usize,
    pub action: usize,
    pub predicted: Option<usize>,
    pub epoch: u64,
    pub observed: usize,
    pub realized_action: usize,
    pub primary: Option<bool>,
    pub secondary: Option<bool>,
    pub energy_before_collection: u64,
    pub collected: Option<u64>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct Counters {
    pub recognition_updates: u64,
    pub model_updates: u64,
    pub detected_errors: u64,
    pub checker_disagreements: u64,
    pub checker_repairs: u64,
    pub checked: u64,
    pub harvests: u64,
    pub blocked_energy: u64,
    pub blocked_symbols: u64,
    pub actions: u64,
}

/// Invariant-preserving state. The read-only snapshot is not an import authority.
#[derive(Clone, Debug)]
pub struct System {
    pub(super) config: Config,
    pub(super) rng: u64,
    pub(super) epoch: u64,
    pub(super) attempts: u64,
    pub(super) energy: u64,
    pub(super) source: u64,
    pub(super) spent: u64,
    pub(super) initial_energy: u64,
    pub(super) background_until: u64,
    pub(super) controls: Controls,
    pub(super) pools: [Vec<Option<usize>>; 2],
    pub(super) ages: [Vec<Option<u64>>; 2],
    pub(super) counts: [Vec<u64>; 2],
    pub(super) reserves: [[u64; 2]; 2],
    pub(super) initial_symbols: [[u64; 2]; 2],
    pub(super) programs: Vec<Option<Program>>,
    pub(super) next_program: u64,
    pub(super) created: [u64; 3],
    pub(super) removed: [u64; 3],
    pub(super) blueprints: [Opcode; 2],
    pub(super) core: [usize; 2],
    pub(super) operators: Vec<Operator>,
    pub(super) model: BTreeMap<(usize, usize), Model>,
    pub(super) production: Vec<Vec<i8>>,
    pub(super) production_order: Vec<usize>,
    pub(super) next: Vec<[usize; 2]>,
    pub(super) at: usize,
    pub(super) target: usize,
    pub(super) version: u64,
    pub(super) history: Vec<Witness>,
    pub(super) events: Vec<Event>,
    pub(super) event_cursor: usize,
    pub(super) clock: Vec<u64>,
    pub(super) ancestry: Value,
    pub(super) counters: Counters,
}

impl System {
    pub fn new(config: Config, events: Vec<Event>) -> Result<Self, Error> {
        Request {
            config: config.clone(),
            events: events.clone(),
            stages: Vec::new(),
            inheritance: None,
            word: None,
        }
        .validate()?;
        let n = config.n;
        let words = 1 << config.length;
        let stock = (n as f64 * config.length as f64 * config.symbol_factor) as u64;
        let mut s = Self {
            rng: config.seed,
            epoch: 0,
            attempts: 0,
            energy: config.energy,
            source: config.source,
            spent: 0,
            initial_energy: config.energy + config.source,
            background_until: config.background_until,
            controls: Controls::default(),
            pools: std::array::from_fn(|_| vec![None; n]),
            ages: std::array::from_fn(|_| vec![None; n]),
            counts: std::array::from_fn(|_| vec![0; words]),
            reserves: [[stock; 2]; 2],
            initial_symbols: [[0; 2]; 2],
            programs: vec![None; n],
            next_program: 0,
            created: [0; 3],
            removed: [0; 3],
            blueprints: [Opcode::Equal, Opcode::Difference],
            core: [0; 2],
            operators: Vec::new(),
            model: BTreeMap::new(),
            production: vec![Vec::new(); words],
            production_order: Vec::new(),
            next: vec![[0; 2]; config.rooms],
            at: 0,
            target: 0,
            version: 0,
            history: Vec::new(),
            events,
            event_cursor: 0,
            clock: vec![config.seed],
            ancestry: Value::Null,
            counters: Counters::default(),
            config,
        };
        let columns = s
            .config
            .columns
            .clone()
            .unwrap_or_else(|| (0..s.config.length).collect());
        for (index, word) in [1, 3].iter().enumerate() {
            let bits = s.bits(*word);
            s.core[index] = columns.iter().fold(0, |v, c| 2 * v + bits[*c]);
        }
        for word in 0..words {
            let mapping = if word == s.core[0] {
                Some((0, 0))
            } else if word == s.core[1] {
                Some((1, 1))
            } else if word == (words - 1) ^ s.core[0] {
                Some((0, 1))
            } else if word == (words - 1) ^ s.core[1] {
                Some((1, 0))
            } else {
                None
            };
            if let Some((input, output)) = mapping {
                let recognizes = match s.config.recognition {
                    Recognition::Fixed => vec![s.core[1 - input]],
                    Recognition::Generic => (0..words).collect(),
                    _ => vec![s.draw(words)],
                };
                s.operators.push(Operator {
                    word,
                    input,
                    output,
                    recognizes,
                });
            }
        }
        let mut order: Vec<_> = (0..s.config.rooms).collect();
        for i in (1..order.len()).rev() {
            let j = s.draw(i + 1);
            order.swap(i, j);
        }
        let flip = s.draw(2);
        for (i, room) in order.iter().enumerate() {
            s.next[*room][flip] = order[(i + 1) % order.len()];
            s.next[*room][1 - flip] = order[(i + order.len() - 1) % order.len()];
        }
        s.at = order[0];
        s.target = order[order.len() / 2];
        if s.config.initial != Initial::Vacant {
            for i in 0..n {
                let word = if s.config.initial == Initial::Core {
                    s.core[i % 2]
                } else {
                    i % words
                };
                s.replace(0, i, Some(word), true);
                s.replace(1, i, Some(0), true);
            }
        }
        s.initial_symbols = s.inventory();
        s.audit()?;
        Ok(s)
    }

    pub(super) fn bits(&self, word: usize) -> Vec<usize> {
        (0..self.config.length)
            .rev()
            .map(|shift| (word >> shift) & 1)
            .collect()
    }
    fn draw(&mut self, n: usize) -> usize {
        let limit = (1u128 << 64) - ((1u128 << 64) % (n as u128));
        loop {
            let mut x = self.rng;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.rng = x;
            if (x as u128) < limit {
                return (x % (n as u64)) as usize;
            }
        }
    }
    fn pay(&mut self) -> bool {
        if self.energy == 0 {
            self.counters.blocked_energy += 1;
            false
        } else {
            self.energy -= 1;
            self.spent += 1;
            true
        }
    }
    pub(super) fn inventory(&self) -> [[u64; 2]; 2] {
        let mut result = self.reserves;
        for (pool, total) in result.iter_mut().enumerate() {
            for word in self.pools[pool].iter().flatten() {
                for bit in self.bits(*word) {
                    total[bit] += 1;
                }
            }
        }
        result
    }
    pub(super) fn weights(&self) -> [[u64; 2]; 2] {
        let mut weights = [[1; 2]; 2];
        if self.controls.feedback {
            for op in &self.operators {
                weights[op.input][op.output] += self.counts[1][op.word];
            }
        }
        weights
    }
    fn replace(
        &mut self,
        pool: usize,
        position: usize,
        word: Option<usize>,
        initial: bool,
    ) -> bool {
        let old = self.pools[pool][position];
        let mut reserve = self.reserves[pool];
        if let Some(old) = old {
            for bit in self.bits(old) {
                reserve[bit] += 1;
            }
        }
        if let Some(word) = word {
            for bit in self.bits(word) {
                if reserve[bit] == 0 {
                    self.counters.blocked_symbols += 1;
                    return false;
                }
                reserve[bit] -= 1;
            }
        }
        self.reserves[pool] = reserve;
        if let Some(old) = old {
            self.counts[pool][old] -= 1;
            self.removed[pool] += 1;
        }
        self.pools[pool][position] = word;
        self.ages[pool][position] = None;
        if pool == 1 && self.programs[position].take().is_some() {
            self.removed[2] += 1;
        }
        if let Some(word) = word {
            self.counts[pool][word] += 1;
            self.created[pool] += 1;
            self.ages[pool][position] = Some(if initial { 0 } else { self.attempts + 1 });
            if pool == 1 && self.controls.renew_checkers {
                if let Some(op) = self.operators.iter().find(|op| op.word == word) {
                    self.programs[position] = Some(Program {
                        id: self.next_program,
                        role: op.input,
                        code: self.blueprints[op.input],
                        created: self.attempts + 1,
                        operator: word,
                    });
                    self.next_program += 1;
                    self.created[2] += 1;
                }
            }
        }
        true
    }
    fn population_step(&mut self) {
        let rates = [
            100,
            if self.controls.copying { 400 } else { 0 },
            5,
            5,
            if self.epoch < self.background_until {
                10
            } else {
                0
            },
        ];
        let mut z = self.draw(rates.iter().sum());
        let mut channel = 0;
        while z >= rates[channel] {
            z -= rates[channel];
            channel += 1;
        }
        self.attempts += 1;
        let n = self.config.n;
        if channel == 2 || channel == 3 {
            let position = self.draw(n);
            self.replace(channel - 2, position, None, false);
            return;
        }
        if !self.pay() {
            return;
        }
        match channel {
            0 => {
                let position = self.draw(n);
                let Some(source) = self.pools[0][position] else {
                    return;
                };
                let weights = self.weights();
                let mut word = 0;
                for bit in self.bits(source) {
                    let a = usize::from(
                        self.draw((weights[bit][0] + weights[bit][1]) as usize)
                            >= weights[bit][0] as usize,
                    );
                    word = 2 * word + a;
                }
                let position = self.draw(n);
                if self.replace(1, position, Some(word), false) {
                    if self.production[source].is_empty() {
                        self.production_order.push(source);
                    }
                    let role = self
                        .operators
                        .iter()
                        .find(|op| op.word == word)
                        .map_or(-1, |op| op.input as i8);
                    let log = &mut self.production[source];
                    log.push(role);
                    if log.len() > 128 {
                        log.remove(0);
                    }
                }
            }
            1 => {
                let position = self.draw(n);
                let source = self.pools[0][position];
                let position = self.draw(n);
                let operator = self.pools[1][position];
                if let (Some(source), Some(operator)) = (source, operator) {
                    if self
                        .operators
                        .iter()
                        .any(|op| op.word == operator && op.recognizes.contains(&source))
                    {
                        let position = self.draw(n);
                        self.replace(0, position, Some(source), false);
                    }
                }
            }
            _ => {
                let position = self.draw(n);
                let word = self.draw(1 << self.config.length);
                self.replace(0, position, Some(word), false);
            }
        }
    }
    fn programs_by_role(&self) -> [Vec<usize>; 2] {
        let mut result = [Vec::new(), Vec::new()];
        for (i, p) in self.programs.iter().enumerate() {
            if let Some(p) = p {
                result[p.role].push(i);
            }
        }
        result
    }
    fn revise_recognition(&mut self, programs: &[Vec<usize>; 2]) {
        if !self.controls.revise_recognition
            || matches!(
                self.config.recognition,
                Recognition::Fixed | Recognition::Generic
            )
            || programs.iter().any(Vec::is_empty)
        {
            return;
        }
        for i in 0..self.operators.len() {
            let needed = (1 - self.operators[i].input) as i8;
            let target = if self.config.recognition == Recognition::Random {
                self.draw(1 << self.config.length)
            } else {
                let mut best: Vec<(usize, usize, usize)> = Vec::new();
                for target in &self.production_order {
                    let log = &self.production[*target];
                    if log.len() < 12 || self.counts[0][*target] == 0 {
                        continue;
                    }
                    let hit = 1 + log.iter().filter(|v| **v == needed).count();
                    let total = log.len() + 2;
                    if best.is_empty() || hit * best[0].1 > best[0].0 * total {
                        best = vec![(hit, total, *target)];
                    } else if hit * best[0].1 == best[0].0 * total {
                        best.push((hit, total, *target));
                    }
                }
                if best.is_empty() {
                    continue;
                }
                let (hit, total, target) = best[self.draw(best.len())];
                let current = self.operators[i].recognizes[0];
                let old = &self.production[current];
                let old_hits = 1 + old.iter().filter(|v| **v == needed).count();
                if old.len() >= 12
                    && self.counts[0][current] > 0
                    && 20 * hit * (old.len() + 2) <= (20 * old_hits + old.len() + 2) * total
                {
                    current
                } else {
                    target
                }
            };
            if self.operators[i].recognizes != [target] {
                self.operators[i].recognizes = vec![target];
                self.counters.recognition_updates += 1;
            }
        }
    }
    fn plan(&self) -> Option<Vec<usize>> {
        let mut queue = VecDeque::from([(self.at, Vec::new())]);
        let mut seen = BTreeSet::from([self.at]);
        while let Some((state, path)) = queue.pop_front() {
            if state == self.target {
                return Some(path);
            }
            for action in 0..2 {
                if let Some(record) = self.model.get(&(state, action)) {
                    if seen.insert(record.target) {
                        let mut next_path = path.clone();
                        next_path.push(action);
                        queue.push_back((record.target, next_path));
                    }
                }
            }
        }
        None
    }
    fn checker(&self, slot: usize, predicted: Option<usize>, observed: usize) -> Option<bool> {
        self.programs[slot].as_ref().map(|p| match p.code {
            Opcode::Equal => predicted == Some(observed),
            Opcode::Difference => BTreeSet::from([predicted])
                .symmetric_difference(&BTreeSet::from([Some(observed)]))
                .next()
                .is_none(),
            Opcode::Accept => true,
        })
    }
    fn act(&mut self, programs: &[Vec<usize>; 2]) {
        if !self.pay() {
            return;
        }
        let before = self.at;
        let target = self.target;
        let path = if self.controls.use_model && programs.iter().all(|p| !p.is_empty()) {
            self.plan()
        } else {
            None
        };
        let unseen: Vec<_> = (0..2)
            .filter(|a| !self.model.contains_key(&(before, *a)))
            .collect();
        let mut action = if !unseen.is_empty() {
            unseen[self.draw(unseen.len())]
        } else if let Some(path) = path.filter(|p| !p.is_empty()) {
            path[0]
        } else {
            self.draw(2)
        };
        if unseen.is_empty() && self.epoch % 4 == 0 {
            action = usize::from(
                self.model[&(before, 1)].last_checked < self.model[&(before, 0)].last_checked,
            );
        }
        let key = (before, action);
        let claim = self.model.get(&key).cloned();
        let predicted = claim.as_ref().map(|c| c.target);
        let row = self.weights()[action];
        let realized = if row[0] == row[1] {
            self.draw(2)
        } else {
            usize::from(row[1] > row[0])
        };
        let after = self.next[before][realized];
        self.at = after;
        let primary = programs[0]
            .first()
            .and_then(|p| self.checker(*p, predicted, after));
        let secondary = if self.controls.independent_check {
            programs[1]
                .first()
                .and_then(|p| self.checker(*p, predicted, after))
        } else {
            None
        };
        if let Some(primary) = primary {
            if secondary.is_some() || !self.controls.independent_check {
                if secondary.is_some_and(|s| s != primary) {
                    self.counters.checker_disagreements += 1;
                    self.blueprints[0] = Opcode::Equal;
                    for slot in &programs[0] {
                        if let Some(p) = &mut self.programs[*slot] {
                            p.code = Opcode::Equal;
                        }
                    }
                    self.counters.checker_repairs += 1;
                } else {
                    self.counters.checked += 1;
                    if predicted.is_some() && !primary {
                        self.counters.detected_errors += 1;
                    }
                    if self.controls.learn && (predicted.is_none() || !primary) {
                        self.model.insert(
                            key,
                            Model {
                                from: before,
                                action,
                                target: after,
                                witness_epoch: self.epoch,
                                witness_clock: self.clock.clone(),
                                last_checked: self.epoch as i64,
                                revision: claim.as_ref().map_or(1, |c| c.revision + 1),
                            },
                        );
                        self.counters.model_updates += 1;
                    } else if primary {
                        if let Some(c) = self.model.get_mut(&key) {
                            c.last_checked = self.epoch as i64;
                        }
                    }
                }
            }
        }
        let energy_before_collection = self.energy;
        let mut collected = None;
        if after == target && self.source > 0 {
            let amount = self.config.packet.min(self.source);
            self.source -= amount;
            self.energy += amount;
            self.counters.harvests += 1;
            let choices: Vec<_> = (0..self.config.rooms).filter(|s| *s != target).collect();
            self.target = choices[self.draw(choices.len())];
            collected = Some(amount);
        }
        self.counters.actions += 1;
        self.history.push(Witness {
            from: before,
            action,
            predicted,
            epoch: self.epoch,
            observed: after,
            realized_action: realized,
            primary,
            secondary,
            energy_before_collection,
            collected,
        });
    }
    fn intervene(&mut self, intervention: &Intervention) {
        match intervention {
            Intervention::EnvironmentSwap {} => {
                for next in &mut self.next {
                    next.swap(0, 1);
                }
                self.version += 1;
            }
            Intervention::CheckerFault { common_mode } => {
                for role in 0..if *common_mode { 2 } else { 1 } {
                    self.blueprints[role] = Opcode::Accept;
                    for p in self
                        .programs
                        .iter_mut()
                        .flatten()
                        .filter(|p| p.role == role)
                    {
                        p.code = Opcode::Accept;
                    }
                }
            }
            Intervention::RecognitionScramble {} => {
                for i in 0..self.operators.len() {
                    let target = self.draw(1 << self.config.length);
                    self.operators[i].recognizes = vec![target];
                }
            }
            Intervention::EraseSequences {} => {
                for i in 0..self.config.n {
                    self.replace(1, i, None, false);
                }
            }
        }
    }
    fn tick(&mut self) {
        while let Some(event) = self
            .events
            .get(self.event_cursor)
            .cloned()
            .filter(|e| e.at <= self.epoch)
        {
            self.intervene(&event.intervention);
            self.event_cursor += 1;
        }
        for _ in 0..self.config.n {
            self.population_step();
        }
        let programs = self.programs_by_role();
        self.revise_recognition(&programs);
        self.act(&programs);
        self.epoch += 1;
    }

    /// Advance bounded stages; budgets are checked before any mutation.
    pub fn run_stages(&mut self, stages: &[Stage]) -> Result<Vec<Value>, Error> {
        let mut candidate = self.clone();
        let measurements = candidate.run_stages_inner(stages)?;
        *self = candidate;
        Ok(measurements)
    }

    fn run_stages_inner(&mut self, stages: &[Stage]) -> Result<Vec<Value>, Error> {
        let total = stages
            .iter()
            .try_fold(self.epoch, |n, s| n.checked_add(s.epochs))
            .ok_or("epoch budget overflow")?;
        if stages.len() > 64 || total > MAX_EPOCHS {
            return Err("organization epoch/stage budget exceeded".into());
        }
        let mut timeline = Vec::new();
        for stage in stages {
            if let Some(controls) = &stage.controls {
                self.controls = controls.clone();
            }
            if stage.drain_resources {
                self.spent += self.energy + self.source;
                self.energy = 0;
                self.source = 0;
            }
            if stage.complement_operators {
                for i in 0..self.config.n {
                    if let Some(word) = self.pools[1][i] {
                        if !self.replace(1, i, Some(word ^ ((1 << self.config.length) - 1)), true) {
                            return Err("insufficient symbols to complement population".into());
                        }
                    }
                }
            }
            for i in 0..stage.epochs {
                self.tick();
                if (i + 1) % 50 == 0 {
                    self.audit()?;
                    timeline.push(self.measurement());
                }
            }
        }
        self.audit()?;
        Ok(timeline)
    }

    pub(super) fn allocate(&mut self, spec: &Inheritance) -> Result<Self, Error> {
        let mut child = self.clone();
        for pool in 0..2 {
            for i in 0..self.config.n {
                child.replace(pool, i, None, false);
            }
        }
        for pool in 0..2 {
            for bit in 0..2 {
                child.reserves[pool][bit] = self.reserves[pool][bit] / 2;
                self.reserves[pool][bit] -= child.reserves[pool][bit];
            }
        }
        child.rng = spec.seed;
        child.epoch = 0;
        child.attempts = 0;
        if !spec.transfer_model {
            child.model.clear();
        }
        child.clock.push(spec.seed);
        for record in child.model.values_mut() {
            record.last_checked -= self.epoch as i64;
        }
        child.history.clear();
        child.events.clear();
        child.event_cursor = 0;
        child.background_until = 0;
        child.counters = Counters::default();
        child.created = [0; 3];
        child.removed = [0; 3];
        for pool in 0..2 {
            if !matches!(
                (pool, spec.mode),
                (_, Transfer::Both) | (0, Transfer::Template) | (1, Transfer::Sequence)
            ) {
                continue;
            }
            let mut positions: Vec<_> = (0..self.config.n).collect();
            for j in 0..spec.bottleneck {
                let k = j + child.draw(positions.len() - j);
                positions.swap(j, k);
            }
            for (j, pos) in positions.iter().take(spec.bottleneck).enumerate() {
                if let Some(word) = self.pools[pool][*pos] {
                    for bit in self.bits(word) {
                        child.reserves[pool][bit] += 1;
                    }
                    if !child.replace(pool, j, Some(word), true) {
                        return Err("allocation conservation failure".into());
                    }
                    self.replace(pool, *pos, None, false);
                    for bit in self.bits(word) {
                        self.reserves[pool][bit] -= 1;
                    }
                }
            }
        }
        child.energy = spec.energy.min(self.energy);
        self.energy -= child.energy;
        child.source = self.source / 2;
        self.source -= child.source;
        child.spent = 0;
        child.initial_energy = child.energy + child.source;
        self.initial_energy -= child.initial_energy;
        child.initial_symbols = child.inventory();
        self.initial_symbols = self.inventory();
        child.ancestry = json!({"parent_epoch":self.epoch,"bottleneck":spec.bottleneck,
            "model_records":child.model.len(),"mode":spec.mode});
        self.audit()?;
        child.audit()?;
        Ok(child)
    }

    pub fn audit(&self) -> Result<(), Error> {
        if self.energy + self.source + self.spent != self.initial_energy
            || self.inventory() != self.initial_symbols
        {
            return Err("organization resource conservation failure".into());
        }
        for pool in 0..2 {
            let mut counts = vec![0; 1 << self.config.length];
            for word in self.pools[pool].iter().flatten() {
                counts[*word] += 1;
            }
            if counts != self.counts[pool] {
                return Err("organization count mismatch".into());
            }
        }
        let mut ids = BTreeSet::new();
        for (slot, p) in self.programs.iter().enumerate() {
            if let Some(p) = p {
                if !ids.insert(p.id) || self.pools[1][slot] != Some(p.operator) {
                    return Err("organization checker ownership mismatch".into());
                }
            }
        }
        Ok(())
    }
}
