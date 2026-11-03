//! SQLite-backed [`RepoManager`] adapter.
//!
//! [`SqliteDb`] owns the physical storage: the `repositories` table, the
//! connection pool and the base directory where clones live. This module adds
//! the *git* half of the [`RepoManager`] contract on top of it:
//!
//! - clone URLs are validated before anything touches the filesystem;
//! - the destination directory is derived from the URL and can never escape
//!   the configured repositories directory;
//! - re-cloning an already registered repository is idempotent (the existing
//!   working copy is reused instead of failing on a non-empty directory);
//! - all metadata is persisted through [`SqliteDb`], so listing reads the same
//!   source of truth.
//!
//! `git2` and `std::fs` are synchronous, so every clone runs inside
//! [`tokio::task::spawn_blocking`] and never stalls the async runtime.
//!
//! [`SqliteDb`] also implements [`RepoManager`] directly (the raw git call
//! without validation or idempotency). Prefer this adapter for application
//! code: it is the hexagonal entry point and the one whose guarantees are
//! covered by tests.

use std::path::{Path, PathBuf};

use anyhow::Context;
use async_trait::async_trait;
use uuid::Uuid;

use crate::db::SqliteDb;
use crate::ports::outbound::repo_manager::{RepoManager, Repository};

/// Name of the directory inside a clone that identifies it as a git
/// repository.
const GIT_DIR: &str = ".git";

/// Normalizes a git URL for comparison purposes.
///
/// Two URLs that only differ in a trailing slash or a `.git` suffix point at
/// the same remote, so both are reduced to the same key.
fn canonical_url(url: &str) -> String {
    url.trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

/// Derives a safe repository name from a git URL (`.../owner/repo.git` ->
/// `repo`).
///
/// Returns `None` when no usable name can be derived (empty input, or a URL
/// whose last segment is `.` / `..`, which would let the clone escape the base
/// directory).
fn repo_name_from_url(url: &str) -> Option<String> {
    let name = url
        .trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()?
        .trim_end_matches(".git")
        .trim();

    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return None;
    }

    Some(name.to_string())
}

/// Replaces every character that is unsafe in a path segment with `-`.
fn sanitize_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Derives the *directory* name used for a clone, incorporating the owner.
///
/// [`repo_name_from_url`] only keeps the last segment, so `owner-a/project.git`
/// and `owner-b/project.git` would both map to `project` — the second clone
/// would silently reuse the first owner's checkout. Including the owner keeps
/// the two apart: `owner-a--project` vs `owner-b--project`.
///
/// Returns `None` under the same conditions as [`repo_name_from_url`], and for
/// URLs made only of separators.
fn repo_dir_name_from_url(url: &str) -> Option<String> {
    let repo = repo_name_from_url(url)?;

    let trimmed = url.trim().trim_end_matches('/');
    let mut segments = trimmed.rsplit('/');

    // Consume the repository segment itself, then try to take the owner.
    segments.next()?;
    let owner = segments.next()?;
    let owner = sanitize_segment(owner);
    let owner = owner.trim_matches('-');

    if owner.is_empty() || owner == "." || owner == ".." {
        return Some(sanitize_segment(&repo));
    }

    Some(format!("{owner}--{}", sanitize_segment(&repo)))
}

/// Reads the `origin` remote URL of an existing checkout, if any.
fn existing_origin_url(repo_path: &Path) -> Option<String> {
    let repo = git2::Repository::open(repo_path).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    let url = remote.url()?.to_string();
    Some(url)
}

/// Builds a [`Repository`] record for a clone that has already been materialized
/// on disk.
fn repository_record(url: &str, name: &str, local_path: &Path) -> Repository {
    Repository {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        url: url.to_string(),
        local_path: Some(local_path.to_string_lossy().into_owned()),
    }
}

/// Blocking clone implementation.
///
/// Clones `url` into `db`'s repositories directory, reusing an existing
/// checkout when the destination is already a git repository. The caller is
/// responsible for persisting the returned record.
fn clone_blocking(db: &SqliteDb, url: &str) -> anyhow::Result<Repository> {
    let base = db.repos_dir();

    // `name` is the human-facing repository name recorded in the database;
    // `dir_name` is the collision-free directory on disk (it includes the
    // owner, so two remotes sharing a basename never share a checkout).
    let name = repo_name_from_url(url)
        .ok_or_else(|| anyhow::anyhow!("cannot derive a repository name from URL {url:?}"))?;
    let dir_name = repo_dir_name_from_url(url)
        .ok_or_else(|| anyhow::anyhow!("cannot derive a repository directory from URL {url:?}"))?;

    let local_path: PathBuf = base.join(&dir_name);

    std::fs::create_dir_all(base)
        .with_context(|| format!("failed to create repositories dir {}", base.display()))?;

    if local_path.join(GIT_DIR).exists() {
        // The working copy is already there (for example from a previous run
        // whose database was recreated). Reuse it only when it really is a
        // checkout of *this* remote: reusing a different repository would hand
        // the caller the wrong codebase to read and modify.
        git2::Repository::open(&local_path).with_context(|| {
            format!(
                "destination {} exists but is not a usable git repository",
                local_path.display()
            )
        })?;

        match existing_origin_url(&local_path) {
            Some(existing) if canonical_url(&existing) == canonical_url(url) => {},
            Some(existing) => anyhow::bail!(
                "refusing to reuse {}: it is a checkout of {:?}, not of {:?}",
                local_path.display(),
                existing,
                url
            ),
            None => anyhow::bail!(
                "refusing to reuse {}: the existing checkout has no `origin` remote, \
                 so it cannot be verified as a checkout of {:?}",
                local_path.display(),
                url
            ),
        }
    } else if local_path.exists() {
        anyhow::bail!(
            "destination {} already exists and is not a git repository",
            local_path.display()
        );
    } else {
        git2::Repository::clone(url, &local_path)
            .with_context(|| format!("failed to clone {url} into {}", local_path.display()))?;
    }

    Ok(repository_record(url, &name, &local_path))
}

/// A [`RepoManager`] adapter backed by [`SqliteDb`].
///
/// ```no_run
/// use gestalt_core::adapters::persistence::SqliteRepo;
/// use gestalt_core::ports::outbound::repo_manager::RepoManager;
///
/// # async fn run() -> anyhow::Result<()> {
/// let repo_manager = SqliteRepo::open("/tmp/gestalt.db")?;
/// let repo = repo_manager
///     .clone_repo("https://github.com/iberi22/gestalt.git")
///     .await?;
/// println!("cloned into {:?}", repo.local_path);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SqliteRepo {
    db: SqliteDb,
}

impl SqliteRepo {
    /// Wraps an existing [`SqliteDb`], inheriting its database file and
    /// repositories directory.
    pub fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Opens (or creates) a SQLite database at `path` and uses the default
    /// repositories directory.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(Self::new(SqliteDb::open(path)?))
    }

    /// Opens an in-memory SQLite database. Cloned repositories still land on
    /// disk (in the default repositories directory) because git needs a real
    /// working tree; only the metadata is ephemeral.
    pub fn open_in_memory() -> anyhow::Result<Self> {
        Ok(Self::new(SqliteDb::open_in_memory()?))
    }

    /// Wraps `db` after pointing its clones at `dir`.
    pub fn with_repos_dir(db: SqliteDb, dir: impl AsRef<Path>) -> Self {
        Self::new(db.with_repos_dir(dir))
    }

    /// Returns the underlying database handle, e.g. to share it with the
    /// vector-search adapter.
    pub fn db(&self) -> &SqliteDb {
        &self.db
    }

    /// Returns the base directory where clones are stored.
    pub fn repos_dir(&self) -> &Path {
        self.db.repos_dir()
    }

    /// Looks up a previously cloned repository by its URL.
    ///
    /// URLs are compared after normalization (trailing slashes and `.git`
    /// suffixes are ignored).
    pub async fn find_repo_by_url(&self, url: &str) -> anyhow::Result<Option<Repository>> {
        let wanted = canonical_url(url);
        Ok(self
            .db
            .list_repos()
            .await?
            .into_iter()
            .find(|repo| canonical_url(&repo.url) == wanted))
    }
}

#[async_trait]
impl RepoManager for SqliteRepo {
    async fn clone_repo(&self, url: &str) -> anyhow::Result<Repository> {
        let url = url.trim();
        if url.is_empty() {
            anyhow::bail!("cannot clone a repository from an empty URL");
        }

        // Re-cloning is idempotent: return the existing record untouched.
        if let Some(existing) = self.find_repo_by_url(url).await? {
            return Ok(existing);
        }

        let db = self.db.clone();
        let url = url.to_string();
        let repo = tokio::task::spawn_blocking(move || clone_blocking(&db, &url))
            .await
            .context("repository clone task failed to run")??;

        self.db.insert_repo(&repo).await?;
        Ok(repo)
    }

    async fn list_repos(&self) -> anyhow::Result<Vec<Repository>> {
        self.db.list_repos().await
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Creates a real local git repository with one commit, so clones never
    /// touch the network.
    fn init_source_repo(dir: &Path) {
        let repo = git2::Repository::init(dir).unwrap();
        std::fs::write(dir.join("README.md"), "# fixture\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
    }

    fn fixture() -> (tempfile::TempDir, tempfile::TempDir) {
        let src = tempfile::tempdir().unwrap();
        init_source_repo(src.path());
        let dest = tempfile::tempdir().unwrap();
        (src, dest)
    }

    #[test]
    fn derives_repository_names() {
        assert_eq!(
            repo_name_from_url("https://github.com/iberi22/gestalt.git").as_deref(),
            Some("gestalt")
        );
        assert_eq!(
            repo_name_from_url("/tmp/fixtures/my-repo/").as_deref(),
            Some("my-repo")
        );
        assert_eq!(repo_name_from_url("   "), None);
        assert_eq!(repo_name_from_url("https://host/.."), None);
    }

    #[test]
    fn canonical_url_ignores_suffixes() {
        assert_eq!(
            canonical_url("https://github.com/iberi22/gestalt.git"),
            canonical_url("https://github.com/iberi22/gestalt/")
        );
        assert_ne!(
            canonical_url("https://github.com/iberi22/gestalt"),
            canonical_url("https://github.com/iberi22/xavier")
        );
    }

    #[tokio::test]
    async fn clone_repo_persists_metadata() {
        let (src, dest) = fixture();
        let adapter = SqliteRepo::with_repos_dir(SqliteDb::open_in_memory().unwrap(), dest.path());
        let url = src.path().to_str().unwrap();

        let repo = adapter.clone_repo(url).await.unwrap();

        assert_eq!(repo.url, url);
        assert_eq!(repo.name, repo_name_from_url(url).unwrap());
        let local_path = PathBuf::from(repo.local_path.clone().unwrap());
        assert!(local_path.join("README.md").exists());

        let listed = adapter.list_repos().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, repo.id);
        assert_eq!(listed[0].local_path, repo.local_path);
    }

    #[tokio::test]
    async fn clone_repo_is_idempotent() {
        let (src, dest) = fixture();
        let adapter = SqliteRepo::with_repos_dir(SqliteDb::open_in_memory().unwrap(), dest.path());
        let url = src.path().to_str().unwrap();

        let first = adapter.clone_repo(url).await.unwrap();
        // A trailing slash points at the same remote and must not duplicate it.
        let second = adapter.clone_repo(&format!("{url}/")).await.unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(adapter.list_repos().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn clone_repo_reuses_existing_checkout() {
        let (src, dest) = fixture();
        let url = src.path().to_str().unwrap();

        let first = SqliteRepo::with_repos_dir(SqliteDb::open_in_memory().unwrap(), dest.path());
        first.clone_repo(url).await.unwrap();

        // A fresh database (e.g. after a reset) finds the working copy already
        // on disk and reuses it instead of failing on a non-empty directory.
        let second = SqliteRepo::with_repos_dir(SqliteDb::open_in_memory().unwrap(), dest.path());
        let repo = second.clone_repo(url).await.unwrap();

        assert!(Path::new(repo.local_path.as_ref().unwrap())
            .join(GIT_DIR)
            .exists());
        assert_eq!(second.list_repos().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn clone_repo_rejects_invalid_input() {
        let dest = tempfile::tempdir().unwrap();
        let adapter = SqliteRepo::with_repos_dir(SqliteDb::open_in_memory().unwrap(), dest.path());

        let empty = adapter.clone_repo("   ").await;
        assert!(empty.is_err());
        assert!(empty.unwrap_err().to_string().contains("empty URL"));

        let missing = adapter
            .clone_repo("/nonexistent/definitely-not-a-repo")
            .await;
        assert!(missing.is_err());
        // The git error is wrapped with the destination for debuggability.
        assert!(missing.unwrap_err().to_string().contains("failed to clone"));

        assert!(adapter.list_repos().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn repo_manager_trait_object_works() {
        let (src, dest) = fixture();
        let adapter: std::sync::Arc<dyn RepoManager> = std::sync::Arc::new(
            SqliteRepo::with_repos_dir(SqliteDb::open_in_memory().unwrap(), dest.path()),
        );

        let repo = adapter
            .clone_repo(src.path().to_str().unwrap())
            .await
            .unwrap();
        let listed = adapter.list_repos().await.unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, repo.id);
    }

    /// Regression test: two remotes that share a basename but differ in owner
    /// must not share a checkout.
    #[test]
    fn dir_names_keep_owners_apart() {
        assert_eq!(
            repo_dir_name_from_url("https://github.com/owner-a/project.git").as_deref(),
            Some("owner-a--project")
        );
        assert_eq!(
            repo_dir_name_from_url("https://github.com/owner-b/project.git").as_deref(),
            Some("owner-b--project")
        );
        assert_ne!(
            repo_dir_name_from_url("https://github.com/owner-a/project.git"),
            repo_dir_name_from_url("https://github.com/owner-b/project.git"),
            "two owners sharing a basename must not map to the same directory"
        );
        // A local path still yields a usable, owner-qualified directory.
        assert_eq!(
            repo_dir_name_from_url("/tmp/fixtures/my-repo/").as_deref(),
            Some("fixtures--my-repo")
        );
        assert_eq!(repo_dir_name_from_url("   "), None);
    }

    /// Regression test: cloning two same-named repositories from different
    /// owners must produce two distinct checkouts with the right content.
    #[tokio::test]
    async fn clone_repo_does_not_confuse_owners_with_same_basename() {
        let sources = tempfile::tempdir().unwrap();
        let owner_a = sources.path().join("owner-a").join("project");
        let owner_b = sources.path().join("owner-b").join("project");
        std::fs::create_dir_all(&owner_a).unwrap();
        std::fs::create_dir_all(&owner_b).unwrap();
        init_source_repo(&owner_a);
        init_source_repo(&owner_b);
        // Make the two fixtures distinguishable.
        std::fs::write(owner_a.join("README.md"), "# owner a\n").unwrap();
        std::fs::write(owner_b.join("README.md"), "# owner b\n").unwrap();

        let dest = tempfile::tempdir().unwrap();
        let adapter = SqliteRepo::with_repos_dir(SqliteDb::open_in_memory().unwrap(), dest.path());

        let a = adapter.clone_repo(owner_a.to_str().unwrap()).await.unwrap();
        let b = adapter.clone_repo(owner_b.to_str().unwrap()).await.unwrap();

        let path_a = PathBuf::from(a.local_path.clone().unwrap());
        let path_b = PathBuf::from(b.local_path.clone().unwrap());
        assert_ne!(
            path_a, path_b,
            "both owners were cloned into the same directory"
        );

        // The first checkout must not have been clobbered by the second.
        let origin_a = existing_origin_url(&path_a).expect("owner-a checkout has an origin");
        assert_eq!(
            canonical_url(&origin_a),
            canonical_url(owner_a.to_str().unwrap())
        );
        assert_eq!(adapter.list_repos().await.unwrap().len(), 2);
    }

    /// Regression test: a pre-existing checkout of a *different* remote at the
    /// target directory must be refused, never silently reused.
    #[tokio::test]
    async fn clone_repo_refuses_to_reuse_a_different_remote() {
        let sources = tempfile::tempdir().unwrap();
        let owner_a = sources.path().join("owner-a").join("project");
        std::fs::create_dir_all(&owner_a).unwrap();
        init_source_repo(&owner_a);

        let dest = tempfile::tempdir().unwrap();
        let url_b = "https://github.com/owner-b/project.git";

        // Materialize what the buggy code used to produce: a checkout of
        // owner-a sitting at the directory owner-b would be cloned into.
        let dir_b = dest
            .path()
            .join(repo_dir_name_from_url(url_b).expect("derivable"));
        git2::Repository::clone(owner_a.to_str().unwrap(), &dir_b).unwrap();

        let adapter = SqliteRepo::with_repos_dir(SqliteDb::open_in_memory().unwrap(), dest.path());
        let err = adapter.clone_repo(url_b).await.unwrap_err().to_string();

        assert!(
            err.contains("refusing to reuse"),
            "expected a refusal, got: {err}"
        );
        assert!(adapter.list_repos().await.unwrap().is_empty());
    }
}
