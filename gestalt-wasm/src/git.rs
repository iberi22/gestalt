use async_trait::async_trait;

#[async_trait(?Send)]
pub trait GitPort: Send + Sync {
    async fn clone(&self, url: &str, path: &str) -> Result<(), String>;
    async fn read_file(&self, path: &str) -> Result<String, String>;
    async fn write_file(&self, path: &str, content: &str) -> Result<(), String>;
    async fn commit(&self, message: &str) -> Result<(), String>;
    async fn push(&self) -> Result<(), String>;
}

/// Native implementation using git2 (unix only).
#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    use super::GitPort;
    use async_trait::async_trait;

    pub struct NativeGitPort;

    #[async_trait(?Send)]
    impl GitPort for NativeGitPort {
        async fn clone(&self, url: &str, path: &str) -> Result<(), String> {
            let path = path.to_string();
            let url = url.to_string();
            tokio::task::spawn_blocking(move || {
                git2::Repository::clone(&url, &path)
                    .map_err(|e| format!("Clone: {e}"))?;
                Ok(())
            })
            .await
            .map_err(|e| format!("Join: {e}"))?
        }

        async fn read_file(&self, path: &str) -> Result<String, String> {
            let repo = git2::Repository::open(path)
                .map_err(|e| format!("Open: {e}"))?;
            let head = repo.head().map_err(|e| format!("Head: {e}"))?;
            let tree = head.peel_to_tree().map_err(|e| format!("Tree: {e}"))?;
            // This is a simplified implementation
            Err("read_file not fully implemented".to_string())
        }

        async fn write_file(&self, _path: &str, _content: &str) -> Result<(), String> {
            // Would write to working directory
            Err("write_file not fully implemented".to_string())
        }

        async fn commit(&self, path: &str, message: &str) -> Result<(), String> {
            let path = path.to_string();
            let message = message.to_string();
            tokio::task::spawn_blocking(move || {
                let repo = git2::Repository::open(&path)
                    .map_err(|e| format!("Open: {e}"))?;
                let mut index = repo.index().map_err(|e| format!("Index: {e}"))?;
                let oid = index.write_tree().map_err(|e| format!("Tree: {e}"))?;
                let tree = repo.find_tree(oid).map_err(|e| format!("Find tree: {e}"))?;
                let author = git2::Signature::now("Gestalt Agent", "agent@gestalt.swal")
                    .map_err(|e| format!("Signature: {e}"))?;
                let head = repo.head().ok();
                let parents: Vec<&git2::Commit> = vec![];
                let parent_refs: Vec<&git2::Commit<'_>> = vec![];
                if let Some(head_ref) = head {
                    if let Ok(commit) = head_ref.peel_to_commit() {
                        // Has parent
                        let _ = repo.commit(
                            Some("HEAD"),
                            &author,
                            &author,
                            &message,
                            &tree,
                            &[&commit],
                        );
                    }
                }
                repo.commit(
                    Some("HEAD"),
                    &author,
                    &author,
                    &message,
                    &tree,
                    &parent_refs,
                )
                .map_err(|e| format!("Commit: {e}"))?;
                Ok(())
            })
            .await
            .map_err(|e| format!("Join: {e}"))?
        }

        async fn push(&self, _path: &str) -> Result<(), String> {
            Err("push not fully implemented".to_string())
        }
    }
}

/// WASM implementation placeholder — isomorphic-git via wasm-bindgen FFI.
#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use super::GitPort;
    use async_trait::async_trait;
    use wasm_bindgen::prelude::*;

    pub struct WasmGitPort;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = isomorphiGit)]
        fn clone(url: &str, path: &str) -> js_sys::Promise;
        #[wasm_bindgen(js_namespace = isomorphiGit)]
        fn readFile(path: &str) -> js_sys::Promise;
        #[wasm_bindgen(js_namespace = isomorphiGit)]
        fn writeFile(path: &str, content: &str) -> js_sys::Promise;
        #[wasm_bindgen(js_namespace = isomorphiGit)]
        fn commit(path: &str, message: &str) -> js_sys::Promise;
        #[wasm_bindgen(js_namespace = isomorphiGit)]
        fn push(path: &str, remote: &str, branch: &str) -> js_sys::Promise;
    }

    #[async_trait(?Send)]
    impl GitPort for WasmGitPort {
        async fn clone(&self, url: &str, path: &str) -> Result<(), String> {
            wasm_bindgen_futures::JsFuture::from(clone(url, path))
                .await
                .map_err(|e| format!("Clone: {:?}", e))?;
            Ok(())
        }

        async fn read_file(&self, path: &str) -> Result<String, String> {
            let val = wasm_bindgen_futures::JsFuture::from(readFile(path))
                .await
                .map_err(|e| format!("ReadFile: {:?}", e))?;
            val.as_string().ok_or("ReadFile: no string".to_string())
        }

        async fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
            wasm_bindgen_futures::JsFuture::from(writeFile(path, content))
                .await
                .map_err(|e| format!("WriteFile: {:?}", e))?;
            Ok(())
        }

        async fn commit(&self, path: &str, message: &str) -> Result<(), String> {
            wasm_bindgen_futures::JsFuture::from(commit(path, message))
                .await
                .map_err(|e| format!("Commit: {:?}", e))?;
            Ok(())
        }

        async fn push(&self, path: &str) -> Result<(), String> {
            wasm_bindgen_futures::JsFuture::from(push(path, "origin", "main"))
                .await
                .map_err(|e| format!("Push: {:?}", e))?;
            Ok(())
        }
    }
}
