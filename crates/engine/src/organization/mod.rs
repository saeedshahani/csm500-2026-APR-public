//! Bounded acquisition, renewable checking and inheritance application.
//!
//! This is a Rust application over the rewrite engine, not a second definition
//! of graph admission. Population events use the explicitly separate host law;
//! a learned table crosses into ordinary admitted execution via `learned_model`.
//! State is private and constructed only from checked, bounded requests.

mod observe;
mod state;

use crate::learned_model;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub use state::System;

/// Input errors never publish a partial application result.
pub type Error = learned_model::Error;
pub const MAX_EPOCHS: u64 = 8192;
pub const MAX_RESOURCE: u64 = 1_000_000_000_000;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Recognition {
    #[default]
    Learned,
    Fixed,
    Random,
    Generic,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Initial {
    #[default]
    Uniform,
    Core,
    Vacant,
}

/// Population size, finite resources and supplied production landscape.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub seed: u64,
    pub n: usize,
    pub rooms: usize,
    pub length: usize,
    pub recognition: Recognition,
    pub energy: u64,
    pub source: u64,
    pub packet: u64,
    pub symbol_factor: f64,
    pub initial: Initial,
    pub columns: Option<Vec<usize>>,
    pub background_until: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 810001,
            n: 64,
            rooms: 8,
            length: 4,
            recognition: Recognition::Learned,
            energy: 18000,
            source: 200000,
            packet: 256,
            symbol_factor: 1.0,
            initial: Initial::Uniform,
            columns: None,
            background_until: 500,
        }
    }
}
impl Config {
    fn validate(&self) -> Result<(), Error> {
        if self.seed == 0
            || !(1..=1024).contains(&self.n)
            || !(3..=32).contains(&self.rooms)
            || !(3..=10).contains(&self.length)
            || !self.symbol_factor.is_finite()
            || !(0.0..=100.0).contains(&self.symbol_factor)
            || [self.energy, self.source, self.packet]
                .iter()
                .any(|v| *v > MAX_RESOURCE)
            || self.background_until > MAX_EPOCHS
        {
            return Err("invalid organization configuration or resource bounds".into());
        }
        if let Some(columns) = &self.columns {
            let mut sorted = columns.clone();
            sorted.sort_unstable();
            if sorted != (0..self.length).collect::<Vec<_>>() {
                return Err("columns must permute the sequence positions".into());
            }
        }
        Ok(())
    }
}

/// Declared interventions, rather than arbitrary writes into host state.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intervention {
    EnvironmentSwap {},
    CheckerFault {
        #[serde(default, skip_serializing_if = "is_false")]
        common_mode: bool,
    },
    RecognitionScramble {},
    EraseSequences {},
}
fn is_false(value: &bool) -> bool {
    !value
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Event {
    pub at: u64,
    #[serde(flatten)]
    pub intervention: Intervention,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Controls {
    pub feedback: bool,
    pub learn: bool,
    pub use_model: bool,
    pub renew_checkers: bool,
    pub independent_check: bool,
    pub revise_recognition: bool,
    pub copying: bool,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            feedback: true,
            learn: true,
            use_model: true,
            renew_checkers: true,
            independent_check: true,
            revise_recognition: true,
            copying: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub epochs: u64,
    /// A complete control profile; omitted fields use the normal enabled value.
    #[serde(default)]
    pub controls: Option<Controls>,
    #[serde(default)]
    pub drain_resources: bool,
    #[serde(default)]
    pub complement_operators: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Transfer {
    #[default]
    Both,
    Template,
    Sequence,
    None,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Inheritance {
    pub seed: u64,
    pub bottleneck: usize,
    pub transfer_model: bool,
    pub mode: Transfer,
    pub energy: u64,
    pub stages: Vec<Stage>,
}
impl Default for Inheritance {
    fn default() -> Self {
        Self {
            seed: 1,
            bottleneck: 32,
            transfer_model: true,
            mode: Transfer::Both,
            energy: 4000,
            stages: Vec::new(),
        }
    }
}

/// One bounded parent execution, optional child allocation, and optional native word.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Request {
    pub config: Config,
    pub events: Vec<Event>,
    pub stages: Vec<Stage>,
    pub inheritance: Option<Inheritance>,
    pub word: Option<Vec<u8>>,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            config: Config::default(),
            events: Vec::new(),
            stages: vec![Stage {
                epochs: 100,
                controls: None,
                drain_resources: false,
                complement_operators: false,
            }],
            inheritance: None,
            word: None,
        }
    }
}
impl Request {
    fn validate(&self) -> Result<(), Error> {
        self.config.validate()?;
        if self.events.len() > 64
            || self.events.iter().any(|e| e.at > MAX_EPOCHS)
            || self.events.windows(2).any(|w| w[0].at > w[1].at)
        {
            return Err("events must be ordered, with at most 64 bounded entries".into());
        }
        let child = self.inheritance.as_ref();
        if child
            .is_some_and(|c| c.seed == 0 || c.bottleneck > self.config.n || c.energy > MAX_RESOURCE)
        {
            return Err("invalid inheritance allocation".into());
        }
        let stages = self
            .stages
            .iter()
            .chain(child.into_iter().flat_map(|c| c.stages.iter()));
        let mut total = 0u64;
        let mut count = 0;
        for stage in stages {
            total = total
                .checked_add(stage.epochs)
                .ok_or("epoch budget overflow")?;
            count += 1;
        }
        if total > MAX_EPOCHS || count > 64 {
            return Err("organization request exceeds 8192 epochs or 64 stages".into());
        }
        if self
            .word
            .as_ref()
            .is_some_and(|w| w.len() > 16 || w.iter().any(|a| *a > 1))
        {
            return Err("native word must contain at most 16 binary actions".into());
        }
        Ok(())
    }
}

/// Execute using only Rust. No external process, interpreter or research tree is used.
pub fn execute(request: &Request) -> Result<Value, Error> {
    request.validate()?;
    let mut parent = System::new(request.config.clone(), request.events.clone())?;
    let parent_initial = parent.snapshot();
    let parent_measurements = parent.run_stages(&request.stages)?;
    let parent_graph = parent.snapshot();
    let parent_summary = parent.summary();
    let mut allocation = Value::Null;
    let mut child_report = Value::Null;
    let mut child;
    let endpoint = if let Some(spec) = &request.inheritance {
        child = parent.allocate(spec)?;
        allocation = json!({"parent":parent.snapshot(), "child":child.snapshot()});
        let timeline = child.run_stages(&spec.stages)?;
        child_report =
            json!({"graph":child.snapshot(),"summary":child.summary(),"timeline":timeline});
        &child
    } else {
        &parent
    };
    let native = request
        .word
        .as_ref()
        .map(|word| {
            let model = endpoint.model_request(word);
            let result = learned_model::execute(&model)?;
            let environmental_agreement =
                endpoint.environmental_agreement(word, &result["projected"]);
            Ok::<_, Error>(json!({"model":model, "execution":result,
            "environmental_agreement":environmental_agreement}))
        })
        .transpose()?;
    Ok(
        json!({"schema":"organization-run-v1", "parent":{"initial":parent_initial,
        "graph":parent_graph,"summary":parent_summary,"timeline":parent_measurements},
        "allocation":allocation,"child":child_report,"native":native}),
    )
}
