//! Checked workbench protocol, shared verbatim by native and WebAssembly hosts.
pub mod dataset;
mod protocol;
pub use protocol::*;
#[cfg(target_arch = "wasm32")]
mod browser {
    use super::Workbench;
    use serde::Serialize;
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    pub struct WorkbenchHandle {
        inner: Workbench,
    }
    impl Default for WorkbenchHandle {
        fn default() -> Self {
            Self::new()
        }
    }

    #[wasm_bindgen]
    impl WorkbenchHandle {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            console_error_panic_hook::set_once();
            Self {
                inner: Workbench::default(),
            }
        }
        pub fn dispatch(&mut self, request: &str) -> Result<JsValue, JsValue> {
            self.inner
                .dispatch(request)
                .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
                .map_err(|e| JsValue::from_str(&format!("bridge serialisation: {e}")))
        }
    }
}
