//! Reference-relative storage accounting for verified, identity-bearing states.
//! This is a read-only measurement, not a new trace importer or graph evaluator.
use super::{exhausted, GraphData, Number, Result, VerifiedTrace};
use graph_core::Kind;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{self, Write};

const SNAPSHOT_SCHEMA: &str = "identity-snapshot-json-v1";

#[derive(Serialize)]
struct Envelope<'a> {
    schema: &'static str,
    graph: &'a GraphData,
}

/// Byte counts concern a compact UTF-8 JSON envelope containing the complete
/// GraphData value. They exclude the evaluator, decoder, history and caches.
/// Equality of hashes is diagnostic; it is not a proof of graph isomorphism.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SnapshotCost {
    pub schema: &'static str,
    pub snapshot_format: &'static str,
    pub state: Number,
    pub trace_profile: String,
    pub execution_profile: crate::runner::ExecutionProfile,
    pub recorded_implementation: String,
    pub measuring_implementation: &'static str,
    pub initial_sha256: String,
    pub snapshot_sha256: String,
    pub payload_bytes: Number,
    pub envelope_bytes: Number,
    pub total_bytes: Number,
    pub total_bits: Number,
    pub atoms: Number,
    pub fields: Number,
    pub label_utf8_bytes: Number,
    pub next_identity: Number,
    pub scope: &'static str,
}

/// Count and hash without retaining a second serialized copy of the graph.
struct Counter {
    used: usize,
    limit: usize,
    hash: Sha256,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .used
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| io::Error::other("snapshot byte limit"))?;
        self.hash.update(bytes);
        self.used = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn measure<T: Serialize>(value: &T, limit: usize) -> Result<(usize, String)> {
    let mut sink = Counter {
        used: 0,
        limit,
        hash: Sha256::new(),
    };
    serde_json::to_writer(&mut sink, value)
        .map_err(|e| exhausted(&format!("snapshot measurement: {e}")))?;
    Ok((sink.used, format!("{:x}", sink.hash.finalize())))
}

impl VerifiedTrace {
    /// Measure one already verified state under an explicit byte envelope.
    /// The whole report fails if the snapshot cannot fit. No graph, history or
    /// cached report is modified. State selection uses ordinary checked replay.
    pub fn snapshot_cost(&self, index: usize, max_snapshot_bytes: usize) -> Result<SnapshotCost> {
        let graph = self.state_at(index)?;
        let data = GraphData::from_graph(&graph);
        let (payload, _) = measure(&data, max_snapshot_bytes)?;
        let (total, digest) = measure(
            &Envelope {
                schema: SNAPSHOT_SCHEMA,
                graph: &data,
            },
            max_snapshot_bytes,
        )?;
        let bits = u64::try_from(total)
            .ok()
            .and_then(|n| n.checked_mul(8))
            .ok_or_else(|| exhausted("snapshot bit count overflow"))?;
        let fields = graph.iter().map(|(_, a)| a.fields.len() as u64).sum();
        let labels = graph
            .iter()
            .map(|(_, a)| match &a.kind {
                Kind::Constructor(label)
                | Kind::Record {
                    label: Some(label), ..
                } => label.len() as u64,
                _ => 0,
            })
            .sum();
        Ok(SnapshotCost {
            schema: "snapshot-cost-v1",
            snapshot_format: SNAPSHOT_SCHEMA,
            state: Number(index as u64),
            trace_profile: self.document.profile.clone(),
            execution_profile: self.document.settings.execution_profile.unwrap_or_default(),
            recorded_implementation: self.document.implementation.clone(),
            measuring_implementation: super::IMPLEMENTATION,
            initial_sha256: self.document.initial_sha256.clone(),
            snapshot_sha256: digest,
            payload_bytes: Number(payload as u64),
            envelope_bytes: Number((total - payload) as u64),
            total_bytes: Number(total as u64),
            total_bits: Number(bits),
            atoms: Number(data.atoms.len() as u64),
            fields: Number(fields),
            label_utf8_bytes: Number(labels),
            next_identity: data.next_identity,
            scope: "Exact compact UTF-8 snapshot storage, including format envelope and allocator state. Excludes evaluator/decoder, trace history, caches, source formatting and execution work. Not a complete-deployment cost or a minimality certificate.",
        })
    }
}
