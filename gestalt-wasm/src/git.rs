use async_trait::async_trait;

#[async_trait(?Send)]
pub trait GitPort: Send + Sync {
    async fn clone(&self, url: &str, path: &str) -> Result<(), String>;
    async fn read_file(&self, path: &str) -> Result<String, String>;
    async fn write_file(&self, path: &str, content: &str) -> Result<(), String>;
    async fn commit(&self, message: &str) -> Result<(), String>;
    async fn push(&self) -> Result<(), String>;
}

#[cfg(not(target_arch = "wasm32"))]
pub struct NativeGitPort {
    repo_path: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeGitPort {
    pub fn new<P: AsRef<std::path::Path>>(repo_path: P) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl GitPort for NativeGitPort {
    async fn clone(&self, url: &str, path: &str) -> Result<(), String> {
        git2::Repository::clone(url, path)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn read_file(&self, path: &str) -> Result<String, String> {
        let full_path = self.repo_path.join(path);
        std::fs::read_to_string(full_path).map_err(|e| e.to_string())
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
        let full_path = self.repo_path.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(full_path, content).map_err(|e| e.to_string())
    }

    async fn commit(&self, message: &str) -> Result<(), String> {
        let repo = git2::Repository::open(&self.repo_path).map_err(|e| e.to_string())?;
        let mut index = repo.index().map_err(|e| e.to_string())?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).map_err(|e| e.to_string())?;
        index.write().map_err(|e| e.to_string())?;

        let tree_id = index.write_tree().map_err(|e| e.to_string())?;
        let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

        let signature = git2::Signature::now("Gestalt Agent", "agent@gestalt.local").map_err(|e| e.to_string())?;

        let mut parents = Vec::new();
        if let Ok(head) = repo.head() {
            if let Ok(peeled) = head.resolve() {
                if let Ok(parent) = peeled.peel_to_commit() {
                    parents.push(parent);
                }
            }
        }

        let parents_refs: Vec<&git2::Commit> = parents.iter().collect();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents_refs,
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    async fn push(&self) -> Result<(), String> {
        let repo = git2::Repository::open(&self.repo_path).map_err(|e| e.to_string())?;
        let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;

        let refspec = "refs/heads/main:refs/heads/main";
        let mut options = git2::PushOptions::new();

        remote.push(&[refspec], Some(&mut options)).map_err(|e| e.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = "
    async function getGit() {
        if (typeof globalThis !== 'undefined' && globalThis.git) return globalThis.git;
        if (typeof window !== 'undefined' && window.git) return window.git;
        return await import('isomorphic-git');
    }

    async function getFs() {
        if (typeof globalThis !== 'undefined' && globalThis.fs) return globalThis.fs;
        if (typeof window !== 'undefined' && window.fs) return window.fs;
        return await import('fs');
    }

    export async function js_clone(url, path) {
        const git = await getGit();
        const fs = await getFs();
        await git.clone({ fs, dir: path, url });
    }

    export async function js_read_file(path) {
        const fs = await getFs();
        const fsPromises = fs.promises || fs;
        const data = await fsPromises.readFile(path, 'utf8');
        return data;
    }

    export async function js_write_file(path, content) {
        const fs = await getFs();
        const fsPromises = fs.promises || fs;
        const pathParts = path.split('/');
        if (pathParts.length > 1) {
            const parent = pathParts.slice(0, -1).join('/');
            await fsPromises.mkdir(parent, { recursive: true }).catch(() => {});
        }
        await fsPromises.writeFile(path, content, 'utf8');
    }

    export async function js_commit(path, message) {
        const git = await getGit();
        const fs = await getFs();

        const matrix = await git.statusMatrix({ fs, dir: path });
        for (const row of matrix) {
            const filepath = row[0];
            const headStatus = row[1];
            const workdirStatus = row[2];
            const stageStatus = row[3];

            if (workdirStatus !== headStatus || stageStatus !== workdirStatus) {
                if (workdirStatus === 0) {
                    await git.remove({ fs, dir: path, filepath });
                } else {
                    await git.add({ fs, dir: path, filepath });
                }
            }
        }

        await git.commit({
            fs,
            dir: path,
            message,
            author: {
                name: 'Gestalt Agent',
                email: 'agent@gestalt.local'
            }
        });
    }

    export async function js_push(path) {
        const git = await getGit();
        const fs = await getFs();
        await git.push({
            fs,
            dir: path
        });
    }
")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn js_clone(url: &str, path: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn js_read_file(path: &str) -> Result<String, JsValue>;

    #[wasm_bindgen(catch)]
    async fn js_write_file(path: &str, content: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn js_commit(path: &str, message: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn js_push(path: &str) -> Result<(), JsValue>;
}

#[cfg(target_arch = "wasm32")]
pub struct WasmGitPort {
    repo_path: String,
}

#[cfg(target_arch = "wasm32")]
impl WasmGitPort {
    pub fn new(repo_path: &str) -> Self {
        Self {
            repo_path: repo_path.to_string(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl GitPort for WasmGitPort {
    async fn clone(&self, url: &str, path: &str) -> Result<(), String> {
        js_clone(url, path)
            .await
            .map_err(|e| format!("{:?}", e))
    }

    async fn read_file(&self, path: &str) -> Result<String, String> {
        let full_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{}/{}", self.repo_path, path)
        };
        js_read_file(&full_path)
            .await
            .map_err(|e| format!("{:?}", e))
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
        let full_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{}/{}", self.repo_path, path)
        };
        js_write_file(&full_path, content)
            .await
            .map_err(|e| format!("{:?}", e))
    }

    async fn commit(&self, message: &str) -> Result<(), String> {
        js_commit(&self.repo_path, message)
            .await
            .map_err(|e| format!("{:?}", e))
    }

    async fn push(&self) -> Result<(), String> {
        js_push(&self.repo_path)
            .await
            .map_err(|e| format!("{:?}", e))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_native_git_port_operations() {
        let dir = tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();

        let _repo = git2::Repository::init(&repo_path).unwrap();

        let port = NativeGitPort::new(&repo_path);

        port.write_file("hello.txt", "world").await.unwrap();

        let content = port.read_file("hello.txt").await.unwrap();
        assert_eq!(content, "world");

        port.commit("initial commit").await.unwrap();

        let repo = git2::Repository::open(&repo_path).unwrap();
        let head = repo.head().unwrap();
        let commit = head.peel_to_commit().unwrap();
        assert_eq!(commit.message().unwrap(), "initial commit");
    }
}
