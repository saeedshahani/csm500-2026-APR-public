//! Restricted domain encoding. Domain labels do not extend the carrier schema.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkItem {
    pub id: String,
    pub status: Status,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Processed,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dataset {
    pub schema: String,
    pub policy: Policy,
    pub items: Vec<WorkItem>,
    // Authoring provenance, not a domain field or trusted execution setting.
    #[serde(skip)]
    recipe: Option<(usize, u32)>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    Fixed,
    GuardedEnable,
}
pub fn validate(dataset: &Dataset) -> Result<(), String> {
    if dataset.schema != "work-items-v1" {
        return Err("unsupported dataset schema".into());
    }
    if dataset.items.len() > 1000 {
        return Err("dataset limit: 1000 items".into());
    }
    let mut seen = BTreeSet::new();
    for item in &dataset.items {
        if item.id.is_empty()
            || item.id.len() > 64
            || !item
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(
                "item id must contain 1–64 ASCII letters, digits, hyphens or underscores".into(),
            );
        }
        if !seen.insert(&item.id) {
            return Err(format!("duplicate item id: {}", item.id));
        }
    }
    Ok(())
}
pub fn source(dataset: &Dataset) -> Result<String, String> {
    validate(dataset)?;
    let active = if dataset.policy == Policy::Fixed {
        "active processing_active;"
    } else {
        "// Initially inactive."
    };
    let mut source = format!(
        r#"program {{
rule process {{
 atom z = Nat(); rank rank0 = z; mode mode0 = ModeObject; revision rev;
 {active}
 atom item = PatternVariable[Constructor] Free;
 atom pending = Record[ObjectLink]("pending", item, item);
 atom processed = Record[ObjectLink]("processed", item, item);
 lhs {{ li: item; ls: pending; }} rhs {{ ri: item; rs: processed; }}
}}
"#
    );
    if dataset.policy == Policy::GuardedEnable {
        source.push_str(r#"rule enable_processing {
 atom own_zero = Nat(); atom own_one = Nat(own_zero);
 rank own_rank = own_one; mode own_mode = ModeMeta; revision own_rev; active own_active;
 atom target = PatternVariable[RuleHead] Free;
 atom target_zero = Nat();
 atom target_rank = Record[Rank](target, target_zero);
 atom target_mode = Record[ModeObject](target);
 atom old_revision = Record[Revision](target);
 atom new_revision = Record[Revision](target);
 atom enabled = Record[Active](target);
 atom permit = Constructor("EnableProcessing");
 lhs { lh: target; lz: target_zero; lr: target_rank; lm: target_mode; lv: old_revision; lp: permit; }
 rhs { rh: target; rz: target_zero; rr: target_rank; rm: target_mode; rv: new_revision; ra: enabled; }
}
atom permission = Constructor("EnableProcessing");
"#);
    }
    for (i, item) in dataset.items.iter().enumerate() {
        let status = match item.status {
            Status::Pending => "pending",
            Status::Processed => "processed",
        };
        source.push_str(&format!("atom item_{i} = Constructor(\"WorkItem:{}\");\natom status_{i} = Record[ObjectLink](\"{status}\", item_{i}, item_{i});\n",item.id));
    }
    source.push_str("}\n");
    if let Some((count, seed)) = dataset.recipe {
        source.push_str(&format!(
            "// Dataset recipe: count={count}, seed={seed}, policy={}\n",
            if dataset.policy == Policy::Fixed {
                "fixed"
            } else {
                "guarded_enable"
            }
        ));
    }
    Ok(source)
}
/// Fixed, portable label permutation. It changes authored order, not the rules.
pub fn generate(count: usize, seed: u32, policy: Policy) -> Result<Dataset, String> {
    if count > 1000 {
        return Err("dataset limit: 1000 items".into());
    }
    let mut items: Vec<_> = (0..count)
        .map(|i| WorkItem {
            id: format!("work-{i:04}"),
            status: Status::Pending,
        })
        .collect();
    let mut state = seed;
    for i in (1..count).rev() {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        items.swap(i, (u64::from(state) % (i as u64 + 1)) as usize);
    }
    Ok(Dataset {
        schema: "work-items-v1".into(),
        policy,
        items,
        recipe: Some((count, seed)),
    })
}
