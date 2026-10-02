//! Counterfactuals for inert-constructor abstraction, not runtime garbage collection.
#[allow(dead_code)]
#[path = "meta_cases.rs"]
pub mod meta;
use engine::stability::Contract;
use graph_core::Graph;

pub fn logger(gated: bool, connected: bool) -> Graph {
    let (atoms, lhs, rhs) = if gated {
        (
            "atom item=Constructor(\"Ticket\"); atom done=Record[ObjectLink](\"done\",item,item);",
            "li:item; ld:done;",
            "ri:item; rd:done;",
        )
    } else {
        ("", "", "")
    };
    let (extra, emitted) = if connected {
        (
            "atom history=Record[ObjectLink](\"history\",log,log);",
            "rh:history;",
        )
    } else {
        ("", "")
    };
    parser::load(&format!("program {{rule logger {{atom z=Nat();rank r=z;mode m=ModeObject;revision v;active a;atom log=Constructor(\"Log\");{atoms}{extra}lhs {{{lhs}}} rhs {{{rhs}emit:log;{emitted}}} }} }}"))
        .unwrap().into_graph()
}

pub fn case(name: &str, n: usize) -> (Graph, Option<Contract>) {
    let (mut g, c) = match name {
        "logger" | "connected" => return (logger(false, name == "connected"), None),
        "output" | "gated-output" => {
            let (g, c) = meta::bodies::output();
            (g, Some(c))
        }
        "gated" | "ungated" | "spawn" | "credit-restoration" | "unfunded-spawn" => {
            meta::case(name, n)
        }
        "new-reader" => (
            parser::load(include_str!("../../../../examples/programs/staging.graph"))
                .unwrap()
                .into_graph(),
            None,
        ),
        "literal-reader" | "wildcard-reader" => {
            let pattern = if name == "literal-reader" {
                "Constructor(\"Log\")"
            } else {
                "PatternVariable[Constructor] Free"
            };
            (parser::load(&format!("program {{rule read {{atom z=Nat();rank r=z;mode m=ModeObject;revision v;active a;atom x={pattern};lhs {{lx:x;}} rhs {{rx:x;}}}} atom log=Constructor(\"Log\");}}" )).unwrap().into_graph(),None)
        }
        _ => panic!("unknown inert case"),
    };
    meta::bodies::append(
        &mut g,
        &logger(matches!(name, "gated" | "gated-output"), false),
        "",
    );
    (g, c)
}
