//! Exact numerical application over the canonical engine, recorder and verifier.
//! Arithmetic is host Rust computation. A native rewrite transports an opaque
//! checked packet; it does not derive the arithmetic or authenticate sensors.
mod domain;
mod publication;
pub use domain::{
    certify, residual_radius, solve, synthesize, Action, Mode, Program, Rational, Task,
};
pub use publication::publish;
pub type Error = crate::learned_model::Error;
use domain::{bound, lipschitz, unit, validate_task};
use num_rational::BigRational as Q;
use num_traits::{One, Signed, Zero};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn digest<T: Serialize>(x: &T) -> Result<String, Error> {
    // Value uses sorted object keys with the workspace's serde_json configuration.
    let value = serde_json::to_value(x).map_err(|_| "serialization failure")?;
    let bytes = serde_json::to_vec(&value).map_err(|_| "serialization failure")?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn natural(n: u64) -> Result<(), Error> {
    if n > 1_000_000_000 {
        Err("logical counter limit".into())
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Certified,
    Uncertified,
    Stale,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Certified => "certified",
            Self::Uncertified => "uncertified",
            Self::Stale => "stale",
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Fault {
    ObservationContract,
    VariationLimit,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReceiptData {
    schema: String,
    identity: String,
    epoch: u64,
    revision: u64,
    task: Task,
    task_digest: String,
    boundary: Rational,
    sequence: u64,
    tick: u64,
    variation: Rational,
    values: Vec<Rational>,
    radius: Rational,
    residual: Rational,
    q: Rational,
    mode: Mode,
    bits: u32,
    steps: usize,
}
/// A receipt handle can only be issued by a live component. Packets alone are not handles.
#[derive(Clone, Debug)]
pub struct Receipt {
    data: ReceiptData,
    key: String,
}
impl Receipt {
    pub fn data(&self) -> &ReceiptData {
        &self.data
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Context {
    epoch: u64,
    revision: u64,
    task_digest: String,
    boundary: Rational,
    active: bool,
    fault: Option<Fault>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    schema: String,
    receipt: ReceiptData,
    receipt_digest: String,
    status: Status,
    radius_now: Option<Rational>,
    tolerance: Rational,
    checked_at: u64,
    max_age: u64,
    observed_sequence: u64,
    observed_variation: Rational,
    current: Context,
}
impl Packet {
    pub fn status(&self) -> Status {
        self.status
    }
}

/// One serialized session. Exclusive mutable access replaces a separate host lock.
/// The registry is not persisted: loading a packet cannot restore session authority.
pub struct Component {
    identity: String,
    epoch: u64,
    revision: u64,
    active: bool,
    fault: Option<Fault>,
    registry: BTreeMap<String, ReceiptData>,
    sequence: u64,
    variation: Q,
    tick: u64,
    task: Task,
    boundary: Rational,
}
impl Component {
    pub fn new(identity: String, task: Task, boundary: Rational, tick: u64) -> Result<Self, Error> {
        if identity.is_empty() || identity.len() > 64 {
            return Err("identity byte limit".into());
        }
        validate_task(&task)?;
        unit(&boundary)?;
        natural(tick)?;
        Ok(Self {
            identity,
            epoch: 0,
            revision: 0,
            active: true,
            fault: None,
            registry: BTreeMap::new(),
            sequence: 0,
            variation: Q::zero(),
            tick,
            task,
            boundary,
        })
    }
    pub fn observe(
        &mut self,
        boundary: Rational,
        sequence: u64,
        tick: u64,
        declared: Option<Rational>,
    ) -> Result<(), Error> {
        if !self.active || self.fault.is_some() {
            return Err("inactive or faulted component".into());
        }
        // A decoded observation is an attempted sample. Semantic failure latches;
        // syntax rejected before this API is not a delivered observation.
        let movement = (&boundary.0 - &self.boundary.0).abs();
        if unit(&boundary).is_err()
            || natural(sequence).is_err()
            || natural(tick).is_err()
            || sequence != self.sequence + 1
            || tick < self.tick
            || declared
                .as_ref()
                .is_some_and(|d| bound(&d.0, 128).is_err() || d.0 < movement)
        {
            self.fault = Some(Fault::ObservationContract);
            return Err("observation contract violation (latched)".into());
        }
        let variation = &self.variation + movement;
        if bound(&variation, 8192).is_err() {
            self.fault = Some(Fault::VariationLimit);
            return Err("variation limit (latched)".into());
        }
        self.variation = variation;
        self.boundary = boundary;
        self.sequence = sequence;
        self.tick = tick;
        Ok(())
    }
    pub fn revise(&mut self, task: Task, boundary: Rational, tick: u64) -> Result<(), Error> {
        validate_task(&task)?;
        unit(&boundary)?;
        natural(tick)?;
        if tick < self.tick || self.revision >= 1_000_000_000 {
            return Err("revision clock or counter limit".into());
        }
        self.task = task;
        self.boundary = boundary;
        self.tick = tick;
        self.revision += 1;
        self.sequence = 0;
        self.variation = Q::zero();
        self.fault = None;
        Ok(())
    }
    pub fn depart(&mut self) {
        self.active = false;
    }
    pub fn rejoin(&mut self, boundary: Rational, tick: u64) -> Result<(), Error> {
        if self.active || self.epoch >= 1_000_000_000 {
            return Err("departure required or incarnation limit".into());
        }
        self.revise(self.task.clone(), boundary, tick)?;
        self.epoch += 1;
        self.active = true;
        Ok(())
    }
    pub fn compute(&mut self, mode: Mode, steps: usize, bits: u32) -> Result<Receipt, Error> {
        if !self.active || self.fault.is_some() {
            return Err("inactive or faulted component".into());
        }
        if self.registry.len() >= 64 {
            return Err("receipt retention limit; new session required".into());
        }
        let values = solve(&self.task, &self.boundary, mode, steps, bits)?;
        let (radius, residual, q) = residual_radius(&self.task, &self.boundary, &values)?;
        let data = ReceiptData {
            schema: "numerical-receipt-v2".into(),
            identity: self.identity.clone(),
            epoch: self.epoch,
            revision: self.revision,
            task: self.task.clone(),
            task_digest: digest(&self.task)?,
            boundary: self.boundary.clone(),
            sequence: self.sequence,
            tick: self.tick,
            variation: Rational(self.variation.clone()),
            values,
            radius,
            residual,
            q,
            mode,
            bits,
            steps,
        };
        let key = digest(&data)?;
        self.registry.insert(key.clone(), data.clone());
        Ok(Receipt { data, key })
    }
    /// Read-only assessment at a supplied logical snapshot. It never advances the
    /// sampled-input clock; invalid/unknown requests cannot expire another receipt.
    /// Repeated assessments are historical claims, not clock-update operations.
    pub fn assess(
        &self,
        receipt: &Receipt,
        tolerance: Rational,
        now: u64,
        max_age: u64,
    ) -> Result<Packet, Error> {
        unit(&tolerance)?;
        natural(now)?;
        natural(max_age)?;
        if self.registry.get(&receipt.key) != Some(&receipt.data) {
            return Err("unknown receipt in this session".into());
        }
        if now < self.tick {
            return Err("assessment precedes latest sampled snapshot".into());
        }
        let mut packet = Packet {
            schema: "numerical-publication-v2".into(),
            receipt: receipt.data.clone(),
            receipt_digest: receipt.key.clone(),
            status: Status::Stale,
            radius_now: None,
            tolerance,
            checked_at: now,
            max_age,
            observed_sequence: self.sequence,
            observed_variation: Rational(self.variation.clone()),
            current: Context {
                epoch: self.epoch,
                revision: self.revision,
                task_digest: digest(&self.task)?,
                boundary: self.boundary.clone(),
                active: self.active,
                fault: self.fault,
            },
        };
        let (status, radius) = decision(&packet)?;
        packet.status = status;
        packet.radius_now = radius.map(Rational);
        verify_packet(&packet)?;
        Ok(packet)
    }
}
fn decision(p: &Packet) -> Result<(Status, Option<Q>), Error> {
    let d = &p.receipt;
    let c = &p.current;
    if p.checked_at < d.tick {
        return Err("publication before computation".into());
    }
    if !c.active
        || c.fault.is_some()
        || d.epoch != c.epoch
        || d.revision != c.revision
        || d.task_digest != c.task_digest
        || p.checked_at - d.tick > p.max_age
    {
        return Ok((Status::Stale, None));
    }
    if p.observed_sequence < d.sequence {
        return Err("sequence regression".into());
    }
    let movement = &p.observed_variation.0 - &d.variation.0;
    if movement < (&c.boundary.0 - &d.boundary.0).abs() {
        return Err("movement underestimates endpoints".into());
    }
    let radius = (&d.radius.0 + lipschitz(&d.task) * movement).min(Q::one());
    bound(&radius, 8192)?;
    Ok((
        if radius <= p.tolerance.0 {
            Status::Certified
        } else {
            Status::Uncertified
        },
        Some(radius),
    ))
}
/// Mathematical consistency only. No signature or hostile-host authentication.
pub fn verify_packet(p: &Packet) -> Result<(), Error> {
    if serde_json::to_vec(p).map_err(|_| "serialization")?.len() > 8192 {
        return Err("packet byte limit".into());
    }
    let d = &p.receipt;
    let c = &p.current;
    if p.schema != "numerical-publication-v2" || d.schema != "numerical-receipt-v2" {
        return Err("unsupported numerical schema".into());
    }
    if d.identity.is_empty()
        || d.identity.len() > 64
        || d.steps > 4096
        || !(4..=64).contains(&d.bits)
    {
        return Err("receipt metadata limits".into());
    }
    for n in [
        p.checked_at,
        p.max_age,
        p.observed_sequence,
        d.tick,
        d.sequence,
        d.epoch,
        d.revision,
        c.epoch,
        c.revision,
    ] {
        natural(n)?;
    }
    for r in [&d.boundary, &c.boundary, &p.tolerance] {
        unit(r)?;
    }
    for r in [&d.variation, &p.observed_variation] {
        bound(&r.0, 8192)?;
        if r.0 < Q::zero() {
            return Err("negative variation".into());
        }
    }
    for hash in [&p.receipt_digest, &d.task_digest, &c.task_digest] {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("digest syntax".into());
        }
    }
    if digest(d)? != p.receipt_digest || digest(&d.task)? != d.task_digest {
        return Err("receipt digest mismatch".into());
    }
    let (radius, residual, q) = residual_radius(&d.task, &d.boundary, &d.values)?;
    if (radius, residual, q) != (d.radius.clone(), d.residual.clone(), d.q.clone()) {
        return Err("incorrect original bound".into());
    }
    let (status, radius) = decision(p)?;
    if status != p.status || radius.map(Rational) != p.radius_now {
        return Err("incorrect status or transported radius".into());
    }
    Ok(())
}
/// Strict byte-bounded ingress; old experimental schemas remain archival only.
pub fn check_packet_json(bytes: &[u8]) -> Result<Packet, Error> {
    if bytes.len() > 8192 {
        return Err("packet byte limit".into());
    }
    let packet: Packet = serde_json::from_slice(bytes).map_err(|e| Error::Input(e.to_string()))?;
    verify_packet(&packet)?;
    Ok(packet)
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Observe {
        boundary: Rational,
        sequence: u64,
        tick: u64,
        #[serde(default)]
        declared_movement: Option<Rational>,
    },
    Compute {
        mode: Mode,
        steps: usize,
        bits: u32,
    },
    Assess {
        receipt: usize,
        tolerance: Rational,
        now: u64,
        max_age: u64,
    },
    Revise {
        task: Task,
        boundary: Rational,
        tick: u64,
    },
    Depart {},
    Rejoin {
        boundary: Rational,
        tick: u64,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub identity: String,
    pub task: Task,
    pub boundary: Rational,
    pub mode: Mode,
    pub steps: usize,
    pub bits: u32,
    pub tolerance: Rational,
    #[serde(default)]
    pub operations: Vec<Operation>,
}
/// Initial publication plus bounded session operations, all in the current process.
pub fn execute(request: &Request) -> Result<Value, Error> {
    if request.operations.len() > 128 {
        return Err("session operation limit".into());
    }
    let mut component = Component::new(
        request.identity.clone(),
        request.task.clone(),
        request.boundary.clone(),
        0,
    )?;
    let first = component.compute(request.mode, request.steps, request.bits)?;
    let packet = component.assess(&first, request.tolerance.clone(), 0, 32)?;
    let mut publications = vec![publish(&packet)?];
    let mut receipts = vec![first];
    let mut retained_bytes = serde_json::to_vec(&publications)
        .map_err(|_| "report serialization")?
        .len();
    for op in &request.operations {
        match op {
            Operation::Observe {
                boundary,
                sequence,
                tick,
                declared_movement,
            } => component.observe(
                boundary.clone(),
                *sequence,
                *tick,
                declared_movement.clone(),
            )?,
            Operation::Compute { mode, steps, bits } => {
                receipts.push(component.compute(*mode, *steps, *bits)?)
            }
            Operation::Assess {
                receipt,
                tolerance,
                now,
                max_age,
            } => {
                let r = receipts
                    .get(*receipt)
                    .ok_or("unknown session receipt index")?;
                let publication =
                    publish(&component.assess(r, tolerance.clone(), *now, *max_age)?)?;
                retained_bytes += serde_json::to_vec(&publication)
                    .map_err(|_| "report serialization")?
                    .len();
                if retained_bytes > 16 * 1024 * 1024 {
                    return Err("numerical report retention limit".into());
                }
                publications.push(publication);
            }
            Operation::Revise {
                task,
                boundary,
                tick,
            } => component.revise(task.clone(), boundary.clone(), *tick)?,
            Operation::Depart {} => component.depart(),
            Operation::Rejoin { boundary, tick } => component.rejoin(boundary.clone(), *tick)?,
        }
    }
    let report = json!({"schema":"numerical-run-v2","publications":publications,"receipts":receipts.iter().map(Receipt::data).collect::<Vec<_>>()});
    if serde_json::to_vec(&report)
        .map_err(|_| "report serialization")?
        .len()
        > 16 * 1024 * 1024
    {
        return Err("numerical report limit".into());
    }
    Ok(report)
}
