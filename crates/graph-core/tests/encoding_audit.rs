//! Bounded representation audit, separate from the selected executable input schema.
use graph_core::{fixture, validate, Binding, Kind, Role};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn default_free_encoding_is_lossless_on_all_counted_fixtures() {
    let mut explicit_tags = BTreeSet::new();
    let mut retained_tags = BTreeSet::new();
    for text in [
        include_str!("../../../examples/fixtures/a2.atoms"),
        include_str!("../../../examples/fixtures/a4.atoms"),
        include_str!("../../../examples/fixtures/b2.atoms"),
        include_str!("../../../examples/fixtures/c1.atoms"),
    ] {
        let fixture = fixture::load(text).unwrap();
        validate(&fixture.graph).unwrap();
        for (_, atom) in fixture.graph.iter() {
            if let Kind::PatternVariable { admits, binding } = &atom.kind {
                let mut attributes = BTreeMap::new();
                match binding {
                    Binding::Free => {
                        explicit_tags.insert("Free");
                    }
                    Binding::AppliedRule => {
                        explicit_tags.insert("AppliedRule");
                        attributes.insert("binding", "AppliedRule");
                    }
                }
                retained_tags.extend(attributes.values().copied());
                let decoded = match attributes.get("binding") {
                    None => Binding::Free,
                    Some(&"AppliedRule") => Binding::AppliedRule,
                    _ => panic!("malformed experimental encoding"),
                };
                assert_eq!(
                    Kind::PatternVariable {
                        admits: admits.clone(),
                        binding: decoded
                    },
                    atom.kind
                );
                // Identities, fields, admitted sets and all non-variable atoms are unchanged.
            }
        }
    }
    assert_eq!(explicit_tags, BTreeSet::from(["Free", "AppliedRule"]));
    assert_eq!(retained_tags, BTreeSet::from(["AppliedRule"]));
    assert_eq!(5 + 8 + 2 + retained_tags.len() + 1, 17);
}

#[test]
fn omitted_control_records_have_only_provisional_storage_savings() {
    let fixtures = [
        (
            include_str!("../../../examples/fixtures/a2.atoms"),
            (30, 38),
            (30, 38),
        ),
        (
            include_str!("../../../examples/fixtures/a4.atoms"),
            (31, 44),
            (30, 43),
        ),
        (
            include_str!("../../../examples/fixtures/b2.atoms"),
            (23, 31),
            (22, 30),
        ),
        (
            include_str!("../../../examples/fixtures/c1.atoms"),
            (81, 117),
            (79, 115),
        ),
    ];
    for (text, without_object_mode, without_active) in fixtures {
        for (role, expected) in [
            (Role::ModeObject, without_object_mode),
            (Role::Active, without_active),
        ] {
            let mut f = fixture::load(text).unwrap();
            validate(&f.graph).unwrap();
            let mut omitted: BTreeSet<_> = f
                .graph
                .iter()
                .filter(|(_, a)| a.kind.role() == Some(role))
                .map(|(id, _)| id)
                .collect();
            loop {
                let previous = omitted.len();
                omitted.extend(
                    f.graph
                        .iter()
                        .filter(|(_, a)| {
                            a.kind.role().is_some_and(Role::membership)
                                && a.fields.iter().any(|t| omitted.contains(t))
                        })
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>(),
                );
                if omitted.len() == previous {
                    break;
                }
            }
            f.graph.remove_set(&omitted).unwrap();
            assert_eq!((f.graph.len(), f.graph.field_count()), expected);
            // This intentionally makes no validity or simulation assertion: deleting a
            // control record also removes the carrier item a reflective LHS could bind.
        }
    }
}
