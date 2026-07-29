//! Gestalt WASM implementation.
//! Provides native and WebAssembly feature gates.

#[cfg(feature = "native")]
pub fn is_native() -> bool {
    true
}

#[cfg(not(feature = "native"))]
pub fn is_native() -> bool {
    false
}

#[cfg(feature = "wasm")]
pub fn is_wasm() -> bool {
    true
}

#[cfg(not(feature = "wasm"))]
pub fn is_wasm() -> bool {
    false
}
