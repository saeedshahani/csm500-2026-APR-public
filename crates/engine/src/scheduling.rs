//! Scheduling is identity-relative, not isomorphism invariant. No fairness claim
//! follows from using a seeded pseudorandom generator.
use crate::PreparedStep;
use graph_core::AtomKey;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub const CANDIDATE_ORDER: &str = "source-template-host-lexicographic-v1";
pub type CandidateKey = (AtomKey, Vec<(AtomKey, AtomKey)>);
pub fn candidate_key(step: &PreparedStep) -> CandidateKey {
    (
        step.binding().source(),
        step.binding()
            .assignments()
            .iter()
            .map(|(a, b)| (*a, *b))
            .collect(),
    )
}

/// Implementations only see completely admitted candidates, in canonical tuple
/// order. The runner rejects an out-of-range index without publishing state.
/// Custom implementations must clone their mutable state independently and
/// perform no external side effects during selection. Rust's Clone trait alone
/// cannot enforce that contract; the supplied Scheduler satisfies it.
/// Selection must also be reproducible from identical cloned state and input;
/// debugger yields may discard and recompute an uncommitted selection.
pub trait SchedulingStrategy: Clone {
    fn supports_count_selection(&self) -> bool {
        false
    }
    fn select_rank(&mut self, _count: usize) -> Option<usize> {
        None
    }
    fn select(&mut self, candidates: &[PreparedStep]) -> Option<usize>;
    fn description(&self) -> String;
    /// Opt in only if selection depends solely on canonical bindings. None
    /// preserves full-candidate access for existing custom strategies.
    fn select_bindings(
        &mut self,
        _candidates: &[crate::OperationalBinding],
    ) -> Option<Option<usize>> {
        None
    }
}

#[derive(Clone, Debug)]
pub enum Scheduler {
    Deterministic,
    Seeded(SeededScheduler),
}
/// Opaque generator state: callers cannot supply a seed inconsistent with the
/// generator whose position is reported.
#[derive(Clone, Debug)]
pub struct SeededScheduler {
    seed: u64,
    rng: Box<ChaCha8Rng>,
}
impl Scheduler {
    pub fn seed(&self) -> Option<u64> {
        match self {
            Self::Deterministic => None,
            Self::Seeded(s) => Some(s.seed),
        }
    }
    /// Decimal u64 at the CLI; the little-endian eight bytes are repeated four
    /// times to form the explicit 256-bit seed. No platform entropy is used.
    pub fn seeded(seed: u64) -> Self {
        let mut bytes = [0; 32];
        for chunk in bytes.chunks_exact_mut(8) {
            chunk.copy_from_slice(&seed.to_le_bytes());
        }
        Self::Seeded(SeededScheduler {
            seed,
            rng: Box::new(ChaCha8Rng::from_seed(bytes)),
        })
    }
}
impl SchedulingStrategy for Scheduler {
    fn supports_count_selection(&self) -> bool {
        true
    }
    fn select_rank(&mut self, count: usize) -> Option<usize> {
        self.select_count(count)
    }
    fn select_bindings(
        &mut self,
        candidates: &[crate::OperationalBinding],
    ) -> Option<Option<usize>> {
        Some(self.select_count(candidates.len()))
    }
    fn select(&mut self, candidates: &[PreparedStep]) -> Option<usize> {
        self.select_count(candidates.len())
    }
    fn description(&self) -> String {
        self.describe()
    }
}
impl Scheduler {
    pub(crate) fn select_count(&mut self, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        match self {
            Self::Deterministic => Some(0),
            Self::Seeded(SeededScheduler { rng, .. }) => {
                let n = count as u64;
                // Explicit unbiased rejection sampling, independent of rand's
                // distribution implementation and of host pointer width.
                let threshold = n.wrapping_neg() % n;
                loop {
                    let value = rng.next_u64();
                    if value >= threshold {
                        return Some((value % n) as usize);
                    }
                }
            }
        }
    }
    fn describe(&self) -> String {
        match self {
            Self::Deterministic => format!("deterministic-v1; order={CANDIDATE_ORDER}; engine={}", env!("CARGO_PKG_VERSION")),
            Self::Seeded(SeededScheduler { seed, rng }) => format!("seeded-v1; seed={seed}; seed-encoding=repeat-le-u64-v1; rng=ChaCha8Rng/rand_chacha-0.3.1/rand-0.8.5; stream={}; word-position={}; sampling=u64-rejection-v1; order={CANDIDATE_ORDER}; engine={}",rng.get_stream(),rng.get_word_pos(),env!("CARGO_PKG_VERSION")),
        }
    }
}
