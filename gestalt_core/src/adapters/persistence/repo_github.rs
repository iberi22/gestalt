use crate::ports::outbound::repo_manager::{RepoManager, Repository};
use async_trait::async_trait;
use octocrab::Octocrab;

pub struct OctoRepoManager {
    octo: Octocrab,
}

impl OctoRepoManager {
    pub fn new(token: String) -> anyhow::Result<Self> {
        let octo = Octocrab::builder().personal_token(token).build()?;
        Ok(Self { octo })
    }
}

#[async_trait]
impl RepoManager for OctoRepoManager {
    async fn clone_repo(&self, url: &str) -> anyhow::Result<Repository> {
        // Here we would use git2 to clone locally
        // For MVP, just wrapping the metadata
        Ok(Repository {
            id: uuid::Uuid::new_v4().to_string(),
            name: url.split('/').next_back().unwrap_or("unknown").to_string(),
            url: url.to_string(),
            local_path: None,
        })
    }

    async fn list_repos(&self) -> anyhow::Result<Vec<Repository>> {
        let repos = self
            .octo
            .current()
            .list_repos_for_authenticated_user()
            .send()
            .await?;

        Ok(repos
            .items
            .into_iter()
            .map(|r| Repository {
                id: r.id.to_string(),
                name: r.name,
                url: r.html_url.map(|u| u.to_string()).unwrap_or_default(),
                local_path: None,
            })
            .collect())
    }
}

impl OctoRepoManager {
    pub async fn close_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> anyhow::Result<()> {
        self.octo
            .issues(owner, repo)
            .update(issue_number)
            .state(octocrab::models::IssueState::Closed)
            .send()
            .await?;
        tracing::info!("Closed issue #{} on {}/{}", issue_number, owner, repo);
        Ok(())
    }

    pub async fn create_pr(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        head: &str,
        base: &str,
    ) -> anyhow::Result<()> {
        self.octo
            .pulls(owner, repo)
            .create(title, head, base)
            .send()
            .await?;
        tracing::info!("Created PR: '{}' on {}/{}", title, owner, repo);
        Ok(())
    }
}
