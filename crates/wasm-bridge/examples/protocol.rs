use std::io::{self, BufRead};
fn main() {
    let mut workbench = wasm_bridge::Workbench::default();
    for line in io::stdin().lock().lines() {
        println!(
            "{}",
            serde_json::to_string(&workbench.dispatch(&line.expect("request line")))
                .expect("reply JSON")
        );
    }
}
