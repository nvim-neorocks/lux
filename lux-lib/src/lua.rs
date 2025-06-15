//! Special utilities for the Lua bridge.

use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};

pub fn lua_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create a new runtime")
    })
}
