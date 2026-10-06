//! Bounded exact Bellman computation and independently checked affine envelopes.
use super::Error;
use num_bigint::BigInt;
use num_rational::BigRational as Q;
use num_traits::{One, Signed, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;

/// Exact rational wire value. JSON floats, Booleans and implicit coercions reject.
/// Inputs use integer or numerator/positive-denominator strings; output is reduced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rational(pub(super) Q);
impl Rational {
    pub fn parse(s: &str) -> Result<Self, Error> {
        if s.len() > 5000 {
            return Err("rational text limit".into());
        }
        let parts: Vec<_> = s.split('/').collect();
        if parts.is_empty() || parts.len() > 2 {
            return Err("rational syntax".into());
        }
        let integer = |s: &str| -> Result<BigInt, Error> {
            let digits = s.strip_prefix('-').unwrap_or(s);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err("rational integer syntax".into());
            }
            s.parse().map_err(|_| "rational integer syntax".into())
        };
        let n = integer(parts[0])?;
        let d = if parts.len() == 2 {
            integer(parts[1])?
        } else {
            BigInt::one()
        };
        if d <= BigInt::zero() || n.bits() > 8192 || d.bits() > 8192 {
            return Err("rational denominator or bit limit".into());
        }
        Ok(Self(Q::new(n, d)))
    }
}
impl Serialize for Rational {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for Rational {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}
pub(super) fn bound(x: &Q, bits: u64) -> Result<(), Error> {
    if x.numer().bits() > bits || x.denom().bits() > bits {
        Err("exact arithmetic bit limit".into())
    } else {
        Ok(())
    }
}
pub(super) fn unit(x: &Rational) -> Result<(), Error> {
    bound(&x.0, 128)?;
    if x.0 < Q::zero() || x.0 > Q::one() {
        Err("value outside unit interval".into())
    } else {
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub weights: Vec<Rational>,
    pub reward: Rational,
    pub boundary: Rational,
}
pub type Task = Vec<Vec<Action>>;
pub type Program = Vec<Vec<(Rational, Rational)>>;

pub fn validate_task(task: &Task) -> Result<(), Error> {
    if task.is_empty() || task.len() > 4 {
        return Err("one to four states required".into());
    }
    let mut policies = 1;
    for row in task {
        if row.is_empty() || row.len() > 16 {
            return Err("action limit".into());
        }
        policies *= row.len();
        if policies > 16 {
            return Err("sixteen-policy limit".into());
        }
        for a in row {
            if a.weights.len() != task.len() {
                return Err("weight dimension".into());
            }
            for x in a.weights.iter().chain([&a.reward, &a.boundary]) {
                unit(x)?;
            }
            let sum: Q = a.weights.iter().map(|w| &w.0).sum();
            if sum >= Q::one() || sum + &a.reward.0 + &a.boundary.0 > Q::one() {
                return Err("invalid substochastic action".into());
            }
        }
    }
    Ok(())
}
fn action(a: &Action, x: &[Q], p: &Q) -> Q {
    &a.reward.0 + &a.boundary.0 * p + a.weights.iter().zip(x).map(|(w, v)| &w.0 * v).sum::<Q>()
}
fn bellman(task: &Task, x: &[Q], p: &Q) -> Result<Vec<Q>, Error> {
    task.iter()
        .map(|row| {
            let y = row
                .iter()
                .map(|a| action(a, x, p))
                .max()
                .ok_or("empty action row")?;
            bound(&y, 8192)?;
            Ok(y)
        })
        .collect()
}
fn policies(task: &Task) -> Vec<Vec<usize>> {
    let mut out = vec![vec![]];
    for row in task {
        out = out
            .iter()
            .flat_map(|prefix| {
                (0..row.len()).map(move |j| {
                    let mut p = prefix.clone();
                    p.push(j);
                    p
                })
            })
            .collect();
    }
    out
}
fn gaussian(mut a: Vec<Vec<Q>>, n: usize) -> Result<Vec<Vec<Q>>, Error> {
    for col in 0..n {
        let pivot = (col..n)
            .find(|r| !a[*r][col].is_zero())
            .ok_or("singular policy")?;
        a.swap(col, pivot);
        let d = a[col][col].clone();
        for v in &mut a[col] {
            *v /= &d;
            bound(v, 8192)?;
        }
        let row = a[col].clone();
        for (r, values) in a.iter_mut().enumerate() {
            if r != col {
                let f = values[col].clone();
                for (v, p) in values.iter_mut().zip(&row) {
                    *v -= &f * p;
                    bound(v, 8192)?;
                }
            }
        }
    }
    Ok(a.into_iter().map(|r| r[n..].to_vec()).collect())
}
fn matrix(task: &Task, policy: &[usize], p: Option<&Q>) -> Vec<Vec<Q>> {
    task.iter()
        .enumerate()
        .map(|(i, row)| {
            let a = &row[policy[i]];
            let mut values: Vec<_> = a
                .weights
                .iter()
                .enumerate()
                .map(|(j, w)| Q::from_integer(BigInt::from(u8::from(i == j))) - &w.0)
                .collect();
            if let Some(p) = p {
                values.push(&a.reward.0 + &a.boundary.0 * p);
            } else {
                values.extend([a.reward.0.clone(), a.boundary.0.clone()]);
            }
            values
        })
        .collect()
}
pub fn synthesize(task: &Task) -> Result<Program, Error> {
    validate_task(task)?;
    let mut lines = vec![BTreeSet::new(); task.len()];
    for policy in policies(task) {
        let solution = gaussian(matrix(task, &policy, None), task.len())?;
        for (row, values) in lines.iter_mut().zip(solution) {
            row.insert((values[0].clone(), values[1].clone()));
        }
    }
    Ok(lines
        .into_iter()
        .map(|r| {
            r.into_iter()
                .map(|(c, d)| (Rational(c), Rational(d)))
                .collect()
        })
        .collect())
}
fn envelope(program: &Program, p: &Q) -> Vec<Q> {
    program
        .iter()
        .map(|r| {
            r.iter()
                .map(|(c, d)| &c.0 + &d.0 * p)
                .max()
                .unwrap_or_default()
        })
        .collect()
}
/// Checks every affine segment without trusting policy enumeration or elimination.
pub fn certify(task: &Task, program: &Program) -> Result<bool, Error> {
    validate_task(task)?;
    if program.len() != task.len() || program.iter().any(|r| r.is_empty() || r.len() > 16) {
        return Ok(false);
    }
    let mut points = BTreeSet::from([Q::zero(), Q::one()]);
    for row in program {
        for (i, (c, d)) in row.iter().enumerate() {
            bound(&c.0, 8192)?;
            bound(&d.0, 8192)?;
            if c.0 < Q::zero() || d.0 < Q::zero() || &c.0 + &d.0 > Q::one() {
                return Ok(false);
            }
            for (e, f) in &row[i + 1..] {
                if d != f {
                    let p = (&e.0 - &c.0) / (&d.0 - &f.0);
                    bound(&p, 8192)?;
                    if p > Q::zero() && p < Q::one() {
                        points.insert(p);
                    }
                }
            }
        }
    }
    let points: Vec<_> = points.into_iter().collect();
    for pair in points.windows(2) {
        let left = envelope(program, &pair[0]);
        let right = envelope(program, &pair[1]);
        for (i, row) in task.iter().enumerate() {
            let mut touching = false;
            for a in row {
                let l = &left[i] - action(a, &left, &pair[0]);
                let r = &right[i] - action(a, &right, &pair[1]);
                bound(&l, 8192)?;
                bound(&r, 8192)?;
                if l < Q::zero() || r < Q::zero() {
                    return Ok(false);
                }
                touching |= l.is_zero() && r.is_zero();
            }
            if !touching {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Raw,
    Compiled,
    Direct,
}
fn rounded(values: Vec<Q>, bits: u32) -> Vec<Q> {
    let scale = BigInt::one() << bits;
    values
        .into_iter()
        .map(|v| Q::new((v.numer() * &scale) / v.denom(), scale.clone()))
        .collect()
}
pub fn solve(
    task: &Task,
    boundary: &Rational,
    mode: Mode,
    steps: usize,
    bits: u32,
) -> Result<Vec<Rational>, Error> {
    validate_task(task)?;
    unit(boundary)?;
    if steps > 4096 || !(4..=64).contains(&bits) {
        return Err("iteration or precision limit".into());
    }
    let p = &boundary.0;
    let values = match mode {
        Mode::Raw => {
            let mut y = vec![Q::zero(); task.len()];
            for _ in 0..steps {
                y = rounded(bellman(task, &y, p)?, bits);
            }
            y
        }
        Mode::Compiled => {
            let program = synthesize(task)?;
            if !certify(task, &program)? {
                return Err("compiler certificate rejected".into());
            }
            rounded(envelope(&program, p), bits)
        }
        Mode::Direct => {
            let mut result = None;
            for policy in policies(task) {
                let y: Vec<_> = gaussian(matrix(task, &policy, Some(p)), task.len())?
                    .into_iter()
                    .map(|r| r[0].clone())
                    .collect();
                if bellman(task, &y, p)? == y {
                    result = Some(rounded(y, bits));
                    break;
                }
            }
            result.ok_or("no optimal policy")?
        }
    };
    Ok(values.into_iter().map(Rational).collect())
}
pub fn residual_radius(
    task: &Task,
    p: &Rational,
    values: &[Rational],
) -> Result<(Rational, Rational, Rational), Error> {
    validate_task(task)?;
    unit(p)?;
    if values.len() != task.len() {
        return Err("output dimension".into());
    }
    for v in values {
        bound(&v.0, 8192)?;
        if v.0 < Q::zero() || v.0 > Q::one() {
            return Err("output outside cube".into());
        }
    }
    let q = task
        .iter()
        .flatten()
        .map(|a| a.weights.iter().map(|w| &w.0).sum::<Q>())
        .max()
        .ok_or("empty task")?;
    let x: Vec<_> = values.iter().map(|v| v.0.clone()).collect();
    let residual = bellman(task, &x, &p.0)?
        .iter()
        .zip(&x)
        .map(|(a, b)| (a - b).abs())
        .max()
        .ok_or("empty values")?;
    let radius = (&residual / (Q::one() - &q)).min(Q::one());
    bound(&radius, 8192)?;
    Ok((Rational(radius), Rational(residual), Rational(q)))
}
pub(super) fn lipschitz(task: &Task) -> Q {
    task.iter()
        .flatten()
        .map(|a| &a.boundary.0 / (Q::one() - a.weights.iter().map(|w| &w.0).sum::<Q>()))
        .max()
        .unwrap_or_default()
}
