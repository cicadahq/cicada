use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub name: String,
    pub url: String,
}

pub async fn git_changed_files() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["status", "--ignored=no", "--short"])
        .output()
        .await?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Failed to get git changed files: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    };

    let changed_files = String::from_utf8(output.stdout)?
        .lines()
        .map(|row| row.trim().split(" ").last().unwrap().to_string())
        .collect();

    Ok(changed_files)
}

pub async fn git_remotes() -> Result<Vec<Origin>> {
    let output = Command::new("git").args(["remote", "-v"]).output().await?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Failed to get git origin: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let origins_str = String::from_utf8(output.stdout)?;

    let mut origins = Vec::new();
    for origin_str in origins_str.lines() {
        let mut origin = origin_str.split_whitespace();
        let name = origin.next().unwrap().to_string();
        let url = origin.next().unwrap().to_string();
        origins.push(Origin { name, url });
    }

    Ok(origins)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Github {
    pub owner: String,
    pub repo: String,
}

impl std::fmt::Display for Github {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)
    }
}

static GITHUB_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"github.com[:/]([a-zA-Z0-9-]+)/([a-zA-Z0-9-]+)").unwrap());

// Tries to use the git remote to find the github repo
async fn github_repo_git() -> Result<Option<Github>> {
    let mut origins = git_remotes().await?;

    // Sort by name to make sure we get the same result every time, first use the weight then by name
    let origin_weight = |origin: &Origin| -> i32 {
        match origin.name.as_str() {
            "upstream" => 3,
            "github" => 2,
            "origin" => 1,
            _ => 0,
        }
    };

    origins.sort_by(|a, b| {
        let weight_a = origin_weight(a);
        let weight_b = origin_weight(b);
        if weight_a == weight_b {
            a.name.cmp(&b.name)
        } else {
            weight_a.cmp(&weight_b)
        }
    });

    // Find the first origin that is a github repo
    let gh = origins.into_iter().find_map(|origin| {
        let captures = GITHUB_REGEX.captures(&origin.url)?;
        let owner = captures.get(1)?.as_str().to_string();
        let repo = captures.get(2)?.as_str().to_string();
        Some(Github { owner, repo })
    });

    Ok(gh)
}

fn github_repo_env() -> Option<Github> {
    match std::env::var("GITHUB_REPOSITORY") {
        Ok(repo) => {
            // Parse the GITHUB_REPOSITORY env var
            let parts: Vec<&str> = repo.split('/').collect();
            if parts.len() == 2 {
                Some(Github {
                    owner: parts[0].into(),
                    repo: parts[1].into(),
                })
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

// Gets the github repo from the GITHUB_REPOSITORY env var or from the git remote
pub async fn github_repo() -> Result<Option<Github>> {
    match github_repo_env() {
        Some(gh_repo) => Ok(Some(gh_repo)),
        None => github_repo_git().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "passing only in the upstream repository: cicadahq/cicada"]
    async fn test_remote_is_github_cicadahq_cicada() {
        let gh = github_repo().await.unwrap().unwrap();
        assert_eq!(gh.owner, "cicadahq");
        assert_eq!(gh.repo, "cicada");
    }
}
