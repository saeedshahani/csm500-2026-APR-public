//! Conservative action acquisition from complete, typed observations.
//!
//! Fitting is an application operation, not a carrier rewrite. Acquired models
//! compile to ordinary object rules; native admission and replay execute them.
//! Safety assumes stationary deterministic conjunctive actions, unconditional
//! effects, injective bindings and the declared parameter-bound frame (SAM).
mod carrier;
pub use crate::learned_model::Error;
pub use carrier::{compile, execute, Compiled};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_FEATURES: usize = 128;
pub const MAX_FACTS: usize = 4096;
pub const MAX_OBSERVATIONS: usize = 4096;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    pub id: String,
    #[serde(deserialize_with = "unique_set")]
    pub types: BTreeSet<String>,
    #[serde(deserialize_with = "unique_map")]
    pub predicates: BTreeMap<String, Vec<String>>,
    #[serde(deserialize_with = "unique_map")]
    pub actions: BTreeMap<String, Vec<String>>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundAtom {
    pub predicate: String,
    pub arguments: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub atom: GroundAtom,
    pub value: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct World {
    #[serde(deserialize_with = "unique_map")]
    pub objects: BTreeMap<String, String>,
    pub facts: Vec<Fact>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub name: String,
    pub arguments: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub schema: String,
    pub environment: String,
    pub expected_version: u64,
    pub epoch: u64,
    pub sequence: u64,
    pub action: Action,
    pub before: World,
    /// None denotes a failed attempt leaving the state unchanged.
    #[serde(deserialize_with = "explicit_after")]
    pub after: Option<World>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repair {
    Local,
    Quarantine,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Safe,
    Rejected,
    Unknown,
    Quarantined,
}
#[derive(Clone, Debug, Serialize)]
pub struct Feature {
    pub predicate: String,
    pub parameters: Vec<usize>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Model {
    pub epoch: u64,
    pub positive_count: usize,
    pub quarantined: bool,
    pub preconditions: Vec<Option<bool>>,
    pub effects: Vec<Option<bool>>,
    post_common: Vec<Option<bool>>,
    failures: BTreeSet<Vec<bool>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub schema: Schema,
    pub environment: String,
    pub version: u64,
    pub evidence_digest: String,
    pub features: BTreeMap<String, Vec<Feature>>,
    pub models: BTreeMap<String, Model>,
}
/// Private mutable state cannot be constructed by deserialising a claimed model.
#[derive(Clone)]
pub struct Learner {
    state: Snapshot,
    policy: Repair,
    observation_limit: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Receipt {
    pub version: u64,
    pub epoch: u64,
    pub alarm: bool,
    pub quarantined: bool,
    pub evidence_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub expected_version: u64,
    pub initial: World,
    pub actions: Vec<Action>,
    pub native_max_work: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: Schema,
    pub environment: String,
    pub repair: Repair,
    pub observations: Vec<Observation>,
    pub run: Option<Run>,
}
pub(super) fn invalid(message: &str) -> Error {
    Error::Input(message.into())
}
fn name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128
}
// A missing result is missing evidence, not an observed failure.
fn explicit_after<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<World>, D::Error> {
    Option::<World>::deserialize(d)
}
fn unique_set<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeSet<String>, D::Error> {
    let values = Vec::<String>::deserialize(d)?;
    let set: BTreeSet<_> = values.iter().cloned().collect();
    if set.len() != values.len() {
        return Err(serde::de::Error::custom("duplicate type"));
    }
    Ok(set)
}
fn unique_map<'de, D, V>(d: D) -> Result<BTreeMap<String, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    V: Deserialize<'de>,
{
    struct Visitor<V>(std::marker::PhantomData<V>);
    impl<'de, V: Deserialize<'de>> serde::de::Visitor<'de> for Visitor<V> {
        type Value = BTreeMap<String, V>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("map with unique keys")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut result = BTreeMap::new();
            while let Some((k, v)) = map.next_entry::<String, V>()? {
                if result.insert(k, v).is_some() {
                    return Err(serde::de::Error::custom("duplicate map key"));
                }
            }
            Ok(result)
        }
    }
    d.deserialize_map(Visitor(std::marker::PhantomData))
}
fn digest<T: Serialize>(value: &T) -> Result<String, Error> {
    let bytes = serde_json::to_vec(value).map_err(|e| invalid(&e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
// Bounded Cartesian product, checked before expanding each partial assignment.
fn product<T: Clone>(choices: &[Vec<T>], bound: usize) -> Result<Vec<Vec<T>>, Error> {
    let mut rows = vec![vec![]];
    for choice in choices {
        if rows.len().saturating_mul(choice.len()) > bound {
            return Err(invalid("grounding bound exceeded"));
        }
        rows = rows
            .into_iter()
            .flat_map(|r| {
                choice.iter().map(move |x| {
                    let mut row = r.clone();
                    row.push(x.clone());
                    row
                })
            })
            .collect();
    }
    Ok(rows)
}
impl Schema {
    fn features(&self) -> Result<BTreeMap<String, Vec<Feature>>, Error> {
        if !name(&self.id)
            || self.types.is_empty()
            || self.types.len() > 16
            || self.predicates.is_empty()
            || self.predicates.len() > 32
            || self.actions.is_empty()
            || self.actions.len() > 16
            || self.types.iter().any(|s| !name(s))
        {
            return Err(invalid("invalid schema names or bounds"));
        }
        for (n, signature) in self.predicates.iter().chain(&self.actions) {
            if !name(n) || signature.len() > 4 || signature.iter().any(|t| !self.types.contains(t))
            {
                return Err(invalid("invalid signature; arity bound is four"));
            }
        }
        let mut result = BTreeMap::new();
        for (action, parameters) in &self.actions {
            let mut features = Vec::new();
            for (predicate, types) in &self.predicates {
                let choices: Vec<_> = types
                    .iter()
                    .map(|t| {
                        parameters
                            .iter()
                            .enumerate()
                            .filter_map(|(i, u)| (u == t).then_some(i))
                            .collect()
                    })
                    .collect();
                for row in product(&choices, MAX_FEATURES)? {
                    if features.len() == MAX_FEATURES {
                        return Err(invalid("local feature bound exceeded"));
                    }
                    features.push(Feature {
                        predicate: predicate.clone(),
                        parameters: row,
                    });
                }
            }
            result.insert(action.clone(), features);
        }
        Ok(result)
    }
    pub(super) fn world(&self, world: &World) -> Result<BTreeMap<GroundAtom, bool>, Error> {
        if world.objects.len() > 64
            || world.facts.len() > MAX_FACTS
            || world
                .objects
                .iter()
                .any(|(n, t)| !name(n) || !self.types.contains(t))
        {
            return Err(invalid("invalid objects or world bounds"));
        }
        let mut expected = BTreeSet::new();
        for (predicate, types) in &self.predicates {
            let choices: Vec<_> = types
                .iter()
                .map(|t| {
                    world
                        .objects
                        .iter()
                        .filter_map(|(n, u)| (u == t).then_some(n.clone()))
                        .collect()
                })
                .collect();
            for arguments in product(&choices, MAX_FACTS)? {
                if expected.len() == MAX_FACTS {
                    return Err(invalid("world grounding bound exceeded"));
                }
                expected.insert(GroundAtom {
                    predicate: predicate.clone(),
                    arguments,
                });
            }
        }
        let mut actual = BTreeMap::new();
        for f in &world.facts {
            if !expected.contains(&f.atom) || actual.insert(f.atom.clone(), f.value).is_some() {
                return Err(invalid("unknown or duplicate fact"));
            }
        }
        if actual.len() != expected.len() {
            return Err(invalid("complete explicit Boolean valuation required"));
        }
        Ok(actual)
    }
}
impl Model {
    fn classify(&self, x: &[bool]) -> Classification {
        if self.quarantined {
            return Classification::Quarantined;
        }
        if self.positive_count == 0 {
            return if self.failures.contains(x) {
                Classification::Rejected
            } else {
                Classification::Unknown
            };
        }
        let violations = |s: &[bool]| -> BTreeSet<usize> {
            self.preconditions
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.is_some_and(|v| s[i] != v).then_some(i))
                .collect()
        };
        let b = violations(x);
        if b.is_empty() {
            return Classification::Safe;
        }
        if self.failures.iter().any(|f| violations(f).is_subset(&b)) {
            Classification::Rejected
        } else {
            Classification::Unknown
        }
    }
    pub(super) fn predict(&self, x: &[bool]) -> Vec<bool> {
        self.effects
            .iter()
            .zip(x)
            .map(|(e, v)| e.unwrap_or(*v))
            .collect()
    }
}
impl Learner {
    pub fn new(schema: Schema, environment: String, policy: Repair) -> Result<Self, Error> {
        Self::with_observation_limit(schema, environment, policy, MAX_OBSERVATIONS)
    }
    /// A caller may lower the publication limit, never raise the application cap.
    pub fn with_observation_limit(
        schema: Schema,
        environment: String,
        policy: Repair,
        limit: usize,
    ) -> Result<Self, Error> {
        if !name(&environment) || limit > MAX_OBSERVATIONS {
            return Err(invalid("invalid environment or observation limit"));
        }
        let features = schema.features()?;
        let models = features
            .keys()
            .map(|a| (a.clone(), Model::default()))
            .collect();
        let evidence_digest = digest(&(&schema, &environment, policy))?;
        Ok(Self {
            state: Snapshot {
                schema,
                environment,
                version: 0,
                evidence_digest,
                features,
                models,
            },
            policy,
            observation_limit: limit,
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.clone()
    }
    pub(super) fn coordinates(
        &self,
        action: &Action,
        world: &World,
    ) -> Result<Vec<GroundAtom>, Error> {
        let signature = self
            .state
            .schema
            .actions
            .get(&action.name)
            .ok_or_else(|| invalid("unknown action"))?;
        if action.arguments.len() != signature.len()
            || action.arguments.iter().collect::<BTreeSet<_>>().len() != action.arguments.len()
            || action
                .arguments
                .iter()
                .zip(signature)
                .any(|(a, t)| world.objects.get(a) != Some(t))
        {
            return Err(invalid("typed injective action binding required"));
        }
        Ok(self.state.features[&action.name]
            .iter()
            .map(|f| GroundAtom {
                predicate: f.predicate.clone(),
                arguments: f
                    .parameters
                    .iter()
                    .map(|i| action.arguments[*i].clone())
                    .collect(),
            })
            .collect())
    }
    pub fn classify(&self, action: &Action, world: &World) -> Result<Classification, Error> {
        let values = self.state.schema.world(world)?;
        let keys = self.coordinates(action, world)?;
        Ok(self.state.models[&action.name]
            .classify(&keys.iter().map(|k| values[k]).collect::<Vec<_>>()))
    }
    /// Validates and stages the entire update before publishing a new version.
    /// External environmental changes are detectable only through new evidence.
    pub fn observe(&mut self, observation: &Observation) -> Result<Receipt, Error> {
        if observation.schema != self.state.schema.id
            || observation.environment != self.state.environment
            || observation.expected_version != self.state.version
            || observation.sequence != self.state.version + 1
        {
            return Err(invalid("stale or foreign observation identity"));
        }
        if self.state.version as usize >= self.observation_limit {
            return Err(invalid("observation publication limit reached"));
        }
        let before = self.state.schema.world(&observation.before)?;
        let keys = self.coordinates(&observation.action, &observation.before)?;
        let x: Vec<_> = keys.iter().map(|k| before[k]).collect();
        let y = if let Some(after) = &observation.after {
            if after.objects != observation.before.objects {
                return Err(invalid("observation object universe changed"));
            }
            let values = self.state.schema.world(after)?;
            let frame: BTreeSet<_> = keys.iter().collect();
            if before
                .iter()
                .any(|(k, v)| !frame.contains(k) && values[k] != *v)
            {
                return Err(invalid("effect outside parameter-bound frame"));
            }
            Some(keys.iter().map(|k| values[k]).collect::<Vec<_>>())
        } else {
            None
        };
        let current = &self.state.models[&observation.action.name];
        if observation.epoch != current.epoch {
            return Err(invalid("stale evidence epoch"));
        }
        let mut next = self.state.clone();
        let model = next
            .models
            .get_mut(&observation.action.name)
            .expect("validated action");
        let class = model.classify(&x);
        let inconsistent = y.as_ref().is_some_and(|y| {
            model
                .effects
                .iter()
                .enumerate()
                .any(|(i, e)| e.is_some_and(|v| v != y[i]))
                || (0..x.len()).any(|i| {
                    x[i] != y[i] && model.positive_count > 0 && model.post_common[i] != Some(y[i])
                })
        });
        let refuted_failure = y.is_some()
            && model.positive_count > 0
            && model.failures.iter().any(|f| {
                model
                    .preconditions
                    .iter()
                    .enumerate()
                    .all(|(i, p)| p.is_none_or(|v| x[i] != v || f[i] == v))
            });
        let alarm = !model.quarantined
            && (inconsistent
                || refuted_failure
                || match (&y, class) {
                    (None, Classification::Safe) | (Some(_), Classification::Rejected) => true,
                    (Some(y), Classification::Safe) => model.predict(&x) != *y,
                    _ => false,
                });
        if alarm {
            match self.policy {
                Repair::Local => {
                    *model = Model {
                        epoch: model.epoch + 1,
                        ..Default::default()
                    }
                }
                Repair::Quarantine => model.quarantined = true,
            }
        }
        if !model.quarantined {
            if let Some(y) = y {
                if model.positive_count == 0 {
                    model.preconditions = x.iter().copied().map(Some).collect();
                    model.post_common = y.iter().copied().map(Some).collect();
                    model.effects = vec![None; x.len()];
                } else {
                    for i in 0..x.len() {
                        if model.preconditions[i] != Some(x[i]) {
                            model.preconditions[i] = None;
                        }
                        if model.post_common[i] != Some(y[i]) {
                            model.post_common[i] = None;
                        }
                    }
                }
                for i in 0..x.len() {
                    if x[i] != y[i] {
                        model.effects[i] = Some(y[i]);
                    }
                }
                model.positive_count += 1;
                // A new positive can refute previously retained failed evidence.
                if model.failures.iter().any(|f| {
                    model
                        .preconditions
                        .iter()
                        .zip(f)
                        .all(|(v, x)| v.is_none_or(|v| v == *x))
                }) {
                    return Err(invalid(
                        "contradictory evidence requires an explicit fresh learner",
                    ));
                }
            } else {
                model.failures.insert(x);
            }
        }
        next.version += 1;
        next.evidence_digest = digest(&(&self.state.evidence_digest, observation, self.policy))?;
        let receipt = Receipt {
            version: next.version,
            epoch: model.epoch,
            alarm,
            quarantined: model.quarantined,
            evidence_digest: next.evidence_digest.clone(),
        };
        self.state = next;
        Ok(receipt)
    }
}
/// Fit a fresh bounded session. A failed batch publishes no externally held learner.
pub fn fit(request: &Request) -> Result<(Learner, Vec<Receipt>), Error> {
    if request.observations.len() > MAX_OBSERVATIONS {
        return Err(invalid("observation bound exceeded"));
    }
    let mut learner = Learner::new(
        request.schema.clone(),
        request.environment.clone(),
        request.repair,
    )?;
    let receipts = request
        .observations
        .iter()
        .map(|o| learner.observe(o))
        .collect::<Result<_, _>>()?;
    Ok((learner, receipts))
}
