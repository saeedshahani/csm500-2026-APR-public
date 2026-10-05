//! Read-only views. Measurements never participate in the transition law.
use super::state::{Model, System, Witness};
use serde_json::{json, Map, Value};

fn state_name(state: usize) -> String {
    format!("e{state}")
}
fn model_json(record: &Model) -> Value {
    json!({"from":state_name(record.from),"action":record.action,"target":state_name(record.target),
        "witness_epoch":record.witness_epoch,"witness_clock":record.witness_clock,
        "last_checked":record.last_checked,"revision":record.revision})
}
fn witness_json(w: &Witness) -> Value {
    let mut result = json!({"from":state_name(w.from),"action":w.action,"predicted":w.predicted.map(state_name),
        "epoch":w.epoch,"observed":state_name(w.observed),"realized_action":w.realized_action,
        "primary":w.primary,"secondary":w.secondary,"energy_before_collection":w.energy_before_collection});
    if let Some(amount) = w.collected {
        result["collected"] = json!(amount);
    }
    result
}
fn tally(counts: [u64; 3]) -> Value {
    json!({"template":counts[0],"sequence":counts[1],"program":counts[2]})
}

impl System {
    /// Immutable JSON projection compatible with the historical scientific oracle.
    /// It carries no authority to restore arbitrary executable state.
    pub fn snapshot(&self) -> Value {
        let model: Map<_, _> = self
            .model
            .iter()
            .map(|((s, a), record)| (format!("e{s}:{a}"), model_json(record)))
            .collect();
        let production: Map<_, _> = self
            .production_order
            .iter()
            .map(|t| (format!("t{t}"), json!(self.production[*t])))
            .collect();
        let pools: Map<_, _> = ["template", "sequence"]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                (
                    name.to_string(),
                    json!(self.pools[i]
                        .iter()
                        .map(|word| word
                            .map(|word| format!("{}{word}", if i == 0 { 't' } else { 'p' })))
                        .collect::<Vec<_>>()),
                )
            })
            .collect();
        let mut root = json!({"tag":"organization","rng":self.rng,"epoch":self.epoch,"attempts":self.attempts,
            "n":self.config.n,"length":self.config.length,"energy":self.energy,"source":self.source,
            "initial_energy":self.initial_energy,"spent_energy":self.spent,"packet":self.config.packet,
            "recognition":self.config.recognition,"rates":[100,if self.controls.copying {400}else{0},5,5,10],
            "basal":1,"feedback":self.controls.feedback,"learn":self.controls.learn,"use_model":self.controls.use_model,
            "renew_checkers":self.controls.renew_checkers,"independent_check":self.controls.independent_check,
            "revise_recognition":self.controls.revise_recognition,"background_until":self.background_until,"pools":pools,
            "ages":{"template":self.ages[0],"sequence":self.ages[1]},
            "reserves":{"template":self.reserves[0],"sequence":self.reserves[1]},
            "initial_symbols":{"template":self.initial_symbols[0],"sequence":self.initial_symbols[1]},
            "programs":self.programs.iter().map(|p|p.as_ref().map(|p|format!("program:{}",p.id))).collect::<Vec<_>>(),
            "created":tally(self.created),"removed":tally(self.removed),"next_program":self.next_program,
            "model":model,"production":production,"production_order":self.production_order.iter().map(|t|format!("t{t}")).collect::<Vec<_>>(),
            "clock_id":self.clock,"history":self.history.iter().map(witness_json).collect::<Vec<_>>(),
            "events":self.events,"event_cursor":self.event_cursor,"ancestry":self.ancestry,"core":self.core,
            "operators":self.operators.iter().map(|op|format!("p{}",op.word)).collect::<Vec<_>>()});
        if let (Some(root), Value::Object(counters)) = (root.as_object_mut(), json!(self.counters))
        {
            root.extend(counters);
        }
        let mut graph = Map::new();
        graph.insert("root".into(), root);
        for role in 0..2 {
            graph.insert(
                format!("blueprint:{role}"),
                json!({"tag":"program_template","instructions":[self.blueprints[role]]}),
            );
        }
        graph.insert(
            "repair:0".into(),
            json!({"tag":"repair_reference","instructions":["equal"]}),
        );
        for word in 0..1 << self.config.length {
            graph.insert(
                format!("t{word}"),
                json!({"tag":"template","sequence":self.bits(word),"count":self.counts[0][word]}),
            );
            let mut node =
                json!({"tag":"sequence","sequence":self.bits(word),"count":self.counts[1][word]});
            if let Some(op) = self.operators.iter().find(|op| op.word == word) {
                node["input_symbol"] = json!(op.input);
                node["output_symbol"] = json!(op.output);
                node["recognizes"] = json!(op
                    .recognizes
                    .iter()
                    .map(|t| format!("t{t}"))
                    .collect::<Vec<_>>());
            }
            graph.insert(format!("p{word}"), node);
        }
        for (i, next) in self.next.iter().enumerate() {
            graph.insert(state_name(i),json!({"tag":"environment_state","next":{"0":state_name(next[0]),"1":state_name(next[1])}}));
        }
        graph.insert("environment".into(),json!({"tag":"environment","states":(0..self.config.rooms).map(state_name).collect::<Vec<_>>(),
            "at":state_name(self.at),"target":state_name(self.target),"version":self.version}));
        for p in self.programs.iter().flatten() {
            graph.insert(format!("program:{}",p.id),json!({"tag":"program_instance",
            "role":p.role,"instructions":[p.code],"created":p.created,"operator":format!("p{}",p.operator)}));
        }
        Value::Object(graph)
    }
    fn decoder(&self) -> [Option<usize>; 2] {
        self.weights().map(|row| {
            if row[0] == row[1] {
                None
            } else {
                Some(usize::from(row[1] > row[0]))
            }
        })
    }
    fn distributions(&self) -> [[f64; 2]; 2] {
        self.decoder().map(|a| match a {
            None => [0.5, 0.5],
            Some(0) => [1.0, 0.0],
            _ => [0.0, 1.0],
        })
    }
    /// Checkpoint measurement, not a guarantee of subsequent survival.
    pub fn measurement(&self) -> Value {
        let weights = self.weights();
        let q = weights.map(|w| {
            [
                w[0] as f64 / (w[0] + w[1]) as f64,
                w[1] as f64 / (w[0] + w[1]) as f64,
            ]
        });
        let fidelity = ((q[0][0] + q[1][1]) / 2.0).max((q[0][1] + q[1][0]) / 2.0);
        let distributions = self.distributions();
        let count = self.model.len();
        let correct: f64 = self
            .model
            .values()
            .map(|m| {
                (0..2)
                    .filter(|a| self.next[m.from][*a] == m.target)
                    .map(|a| distributions[m.action][a])
                    .sum::<f64>()
            })
            .sum();
        let window = &self.history[self.history.len().saturating_sub(100)..];
        let mut roles = Map::new();
        for p in self.programs.iter().flatten() {
            let entry = roles.entry(p.role.to_string()).or_insert(json!(0));
            *entry = json!(entry.as_u64().unwrap_or(0) + 1);
        }
        let mut row = json!({"epoch":self.epoch,"energy":self.energy,"source":self.source,
            "fidelity":fidelity,"code":self.decoder(),"action_distributions":distributions,
            "decoder_deterministic":self.decoder().iter().all(Option::is_some),
            "active_fraction":self.operators.iter().map(|op|self.counts[1][op.word]).sum::<u64>() as f64/self.config.n as f64,
            "core_fractions":self.core.map(|c|self.counts[0][c] as f64/self.config.n as f64),
            "model_records":count,"model_accuracy":if count==0 {0.0} else {correct/count as f64},
            "model_coverage":count as f64/(2*self.config.rooms) as f64,
            "recent_predictions":window.iter().filter(|w|w.predicted.is_some()).count(),
            "recent_correct":window.iter().filter(|w|w.predicted==Some(w.observed)).count(),
            "recent_harvests":window.iter().filter(|w|w.collected.is_some()).count(),
            "original_instances":{"template":self.ages[0].iter().filter(|v|**v==Some(0)).count(),
                "sequence":self.ages[1].iter().filter(|v|**v==Some(0)).count()},
            "program_roles":roles,"program_creations":self.created[2],"program_removals":self.removed[2]});
        if let (Some(row), Value::Object(counters)) = (row.as_object_mut(), json!(self.counters)) {
            row.extend(counters);
        }
        row
    }
    pub fn summary(&self) -> Value {
        let row = self.measurement();
        let functional = row["fidelity"].as_f64().unwrap_or(0.0) >= 0.9
            && row["active_fraction"].as_f64().unwrap_or(0.0) >= 0.35
            && row["core_fractions"]
                .as_array()
                .is_some_and(|v| v.iter().all(|f| f.as_f64().unwrap_or(0.0) >= 0.1))
            && row["model_accuracy"].as_f64().unwrap_or(0.0) >= 0.95
            && row["model_coverage"].as_f64().unwrap_or(0.0) >= 0.75
            && row["recent_harvests"].as_u64().unwrap_or(0) >= 10
            && row["program_roles"]["0"].as_u64().unwrap_or(0) > 0
            && row["program_roles"]["1"].as_u64().unwrap_or(0) > 0;
        json!({"measurement":row,"functional":functional,"heldout":self.heldout()})
    }
    fn heldout(&self) -> Value {
        let distributions = self.distributions();
        let mut expected = 0.0;
        for start in 0..self.config.rooms {
            for word in 0..64 {
                let mut state = start;
                let mut probability = 1.0;
                for shift in (0..6).rev() {
                    let action = (word >> shift) & 1;
                    let Some(record) = self.model.get(&(state, action)) else {
                        probability = 0.0;
                        break;
                    };
                    probability *= (0..2)
                        .filter(|b| self.next[state][*b] == record.target)
                        .map(|b| distributions[action][b])
                        .sum::<f64>();
                    state = record.target;
                }
                expected += probability;
            }
        }
        let deterministic = self.decoder().iter().all(Option::is_some);
        json!({"passed":if deterministic {Some(expected as u64)} else {None},"expected_passed":expected,
            "total":self.config.rooms*64,"deterministic":deterministic,
            "scope":"exact prefix-agreement probability at frozen checkpoint; not continued noisy dynamics"})
    }
    pub(super) fn model_request(&self, word: &[u8]) -> Value {
        json!({"states":(0..self.config.rooms).map(state_name).collect::<Vec<_>>(),"start":state_name(self.at),
            "transitions":self.model.values().map(model_json).collect::<Vec<_>>(),"word":word})
    }
    pub(super) fn environmental_agreement(&self, word: &[u8], projected: &Value) -> Option<bool> {
        let code = self.decoder();
        let code = [code[0]?, code[1]?];
        let mut state = self.at;
        let mut path = vec![state_name(state)];
        for action in word {
            state = self.next[state][code[*action as usize]];
            path.push(state_name(state));
        }
        Some(json!(path) == *projected)
    }
}
