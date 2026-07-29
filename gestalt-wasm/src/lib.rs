pub mod git;
pub use git::GitPort;

#[cfg(not(target_arch = "wasm32"))]
pub use git::NativeGitPort;

#[cfg(target_arch = "wasm32")]
pub use git::WasmGitPort;
