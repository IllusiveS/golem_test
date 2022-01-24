use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use reqwest::Client;

use futures::future;

use anyhow::{Context, Result};

use async_trait::async_trait;

use log::error;

use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoContributorsInfo {
    #[serde(alias = "name")]
    pub login: String,
    pub contributions: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OwnerInfo {
    pub login: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SingleRepoInfo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub stargazers_count: u64,
    pub commits_url: String,
    pub owner: OwnerInfo,
    pub size: u64,
    #[serde(default)]
    pub num_of_commits: u32,
    #[serde(default)]
    pub bus_factor: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoriesInfo {
    pub items: Vec<SingleRepoInfo>,
    pub total_count: u64,
    pub incomplete_results: bool,
}

#[async_trait]
pub trait GithubProvider {
    async fn gather_repositories_info(
        self : Arc<Self>,
        langage: String,
        num_of_repos: u32,
    ) -> Result<Vec<SingleRepoInfo>>;
    async fn gather_single_repository_info(
        self : Arc<Self>,
        repo_info: SingleRepoInfo,
    ) -> Result<(SingleRepoInfo, Vec<RepoContributorsInfo>)>;
}

#[derive(Clone)]
pub struct APIGithubProvider {
    client: Client,
    token: String,
}

impl APIGithubProvider {
    pub fn new(token: String) -> Self {
        APIGithubProvider {
            client: Client::new(),
            token,
        }
    }
}

async fn unpack_responses_from_json<T>(response: reqwest::Response) -> Result<T>
where
    T: DeserializeOwned,
{
    let response_text = response.text().await?;
    
    let parsed_response = serde_json::from_str::<T>(&response_text);

    let parsed_values = parsed_response.context("Failed to parse response into structs")?;

    Ok(parsed_values)
}

#[async_trait]
impl GithubProvider for APIGithubProvider {
    async fn gather_repositories_info(
        self : Arc<Self>,
        lang: String,
        num_of_repos: u32,
    ) -> Result<Vec<SingleRepoInfo>> {
        let required_calls = (num_of_repos / 100) + 1;
        let last_page_num_of_repos = num_of_repos % 100;

        let pages = 1..(required_calls + 1);

        let results_futures : Vec<_> = pages.into_iter().
            map(|page|{
                return self.client.get(format!("https://api.github.com/search/repositories?q=language:{language}&sort=stars&order=desc&page={page_num}&per_page=100", language = lang, page_num = page))
                    .header("Authorization",  format!("token {token_string}", token_string = self.token))
                    .header("User-Agent", "Request")
                    .header("Accept", "application/vnd.github.v3+json")
                    .send()
            }).collect();

        let results: Vec<reqwest::Response> = future::try_join_all(results_futures)
            .await
            .context("Failed to retrieve responses about individual repositories")?;

        let mut responses_parsed = future::try_join_all(
            results
                .into_iter()
                .map(unpack_responses_from_json::<RepositoriesInfo>),
        )
        .await
        .context("Failed to parse all repo info from response")?;

        responses_parsed
            .last_mut()
            .unwrap()
            .items
            .truncate(last_page_num_of_repos.try_into().unwrap());

        let single_repo_vec: Vec<SingleRepoInfo> = responses_parsed
            .into_iter()
            .flat_map(|element| element.items)
            .collect();

        return Ok(single_repo_vec);
    }

    async fn gather_single_repository_info(
        self : Arc<Self>,
        mut repo_info: SingleRepoInfo,
    ) -> anyhow::Result<(SingleRepoInfo, Vec<RepoContributorsInfo>)> {
        let commit_infos_results  = self.client.get(format!("https://api.github.com/repos/{owner}/{repo_name}/contributors?q=anon=false&page=1&per_page=25&anon=true", owner = repo_info.owner.login, repo_name = repo_info.name))
            .header("Authorization",  format!("token {token_string}", token_string = self.token))
            .header("User-Agent", "Request")
            .header("Accept", "application/vnd.github.v3+json")
            .send().await;

        let commit_infos =
            commit_infos_results.context("Failed to get response from github API")?;

        let responses_parsed =
            unpack_responses_from_json::<Vec<RepoContributorsInfo>>(commit_infos)
                .await
                .context("Failed to parse singl repo info from response")
                .unwrap();

        repo_info.num_of_commits = responses_parsed
            .iter()
            .fold(0, |acc, info| acc + info.contributions);

        return Ok((repo_info, responses_parsed));
    }
}
