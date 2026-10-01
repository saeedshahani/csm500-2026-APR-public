#[allow(dead_code)]
#[path = "body_revisions.rs"]
pub mod bodies;
use engine::stability::Contract;
use graph_core::{Graph, Kind, Role};

pub fn case(name: &str, n: usize) -> (Graph, Option<Contract>) {
    match name {
        "gated"=>{let(g,c)=bodies::modulating(true);(g,Some(c))},
        "ungated"=>{let(g,c)=bodies::modulating(false);(g,Some(c))},
        "interleaved"=>{
            let(mut g,c)=bodies::modulating(false);
            let heads:Vec<_>=graph_core::validate(&g).unwrap().rules.values().filter(|r|r.meta).map(|r|r.head).collect();
            for h in heads {
                let permit=g.insert(Kind::Constructor("Permit".into()),&[]).unwrap();
                let wait=g.insert(Kind::Constructor("Wait".into()),&[]).unwrap();
                g.insert(Kind::record(Role::Lhs),&[h,permit]).unwrap();
                g.insert(Kind::record(Role::Rhs),&[h,wait]).unwrap();
            }
            let ordinary=parser::load("program {rule recharge {atom z=Nat();rank r=z;mode m=ModeObject;revision v;active a;atom w=Constructor(\"Wait\");atom p=Constructor(\"Permit\");lhs {l:w;} rhs {rp:p;}}}").unwrap().into_graph();
            bodies::append(&mut g,&ordinary,"");
            g.insert(Kind::Constructor("Permit".into()),&[]).unwrap();
            (g,Some(c))
        },
        "delete-head"=> (parser::load(include_str!("../../../../examples/practical/delete-rule.graph")).unwrap().into_graph(),None),
        "guard"=>{let(g,c)=bodies::guard(n);(g,Some(c))},
        "restoration"=> (parser::load(include_str!("../../../../examples/programs/restoration.graph")).unwrap().into_graph(),None),
        "credit-restoration"=>{
            let(mut g,_)=case("restoration",n);
            let heads:Vec<_>=graph_core::validate(&g).unwrap().rules.values().filter(|r|r.meta).map(|r|r.head).collect();
            for head in heads {
                let credit=g.insert(Kind::Constructor("MetaCredit".into()),&[]).unwrap();
                g.insert(Kind::record(Role::Lhs),&[head,credit]).unwrap();
            }
            for _ in 0..n {g.insert(Kind::Constructor("MetaCredit".into()),&[]).unwrap();}
            (g,None)
        },
        "spawn"|"unfunded-spawn"=>{
            let source=include_str!("../../../../examples/practical/credit-spawn.graph");
            let source=source.replace(" atom permit_a = Constructor(\"MetaCredit\");", "").replace(" atom permit_b = Constructor(\"MetaCredit\");", "");
            let source=if name=="unfunded-spawn"{source.replace("lhs { lc: credit; }","lhs { }")}else{source};
            let mut g=parser::load(&source).unwrap().into_graph();
            for _ in 0..n {g.insert(Kind::Constructor("MetaCredit".into()),&[]).unwrap();}
            (g,None)
        },
        "self"=>{
            let text=include_str!("../../../../examples/programs/self-modify.graph");
            let source=format!("{}\n}}",text.split("    atom supervisor =").next().unwrap());
            (parser::load(&source).unwrap().into_graph(),None)
        },
        "object-loop"=> (parser::load(include_str!("../../../../examples/behaviour/oscillating.graph")).unwrap().into_graph(),None),
        "meta-noop"=> (parser::load("program {rule idle {atom z=Nat();rank r=z;mode m=ModeMeta;revision v;active a;lhs {} rhs {}}}").unwrap().into_graph(),None),
        "finite-then-object-loop"=>{
            let(mut a,_)=case("self",n);let(b,_)=case("object-loop",n);bodies::append(&mut a,&b,"O:");(a,None)
        },
        _=>panic!("unknown research case"),
    }
}
