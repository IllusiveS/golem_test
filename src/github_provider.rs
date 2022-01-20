use serde::{Deserialize, Serialize};

use reqwest::Client;

use futures::executor;
use futures::future;
use futures::future::FutureExt;

use async_trait::async_trait;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoContributorsInfo {
    #[serde(alias = "name")]
    pub login: String,
    pub contributions: u32,
}

pub async fn parse_contributors_info(req: String) -> Vec<RepoContributorsInfo> {
    let repositories_info: Vec<RepoContributorsInfo> = serde_json::from_str::<Vec<RepoContributorsInfo>>(&req).unwrap();
    repositories_info
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OwnerInfo {
    pub login: String,
}

#[derive(Serialize, Deserialize, Debug)]
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

pub fn parse_response(req: String) -> serde_json::Result<RepositoriesInfo> {
    let repositories_info = serde_json::from_str::<RepositoriesInfo>(&req)?;
    Ok(repositories_info)
}

#[async_trait]
pub trait GithubProvider {
    async fn gather_repositories_info(
        &self,
        langage: String,
        num_of_repos: u32,
    ) -> Result<Vec<SingleRepoInfo>, reqwest::Error>;
    async fn gather_single_repository_info(
        &self,
        repo_info: SingleRepoInfo,
    ) -> Result<(SingleRepoInfo, Vec<RepoContributorsInfo>), reqwest::Error>;
}

pub struct APIGithubProvider {
    client: Client,
    token: String,
}

impl APIGithubProvider {
    fn new(token: String) -> Self {
        APIGithubProvider {
            client: Client::new(),
            token: token,
        }
    }
}

#[async_trait]
impl GithubProvider for APIGithubProvider {
    async fn gather_repositories_info(
        &self,
        lang: String,
        num_of_repos: u32,
    ) -> Result<Vec<SingleRepoInfo>, reqwest::Error> {
        let required_calls = (num_of_repos / 100) + 1;
        let last_page_num_of_repos = num_of_repos % 100;

        let pages: Vec<u32> = (1..required_calls + 1).into_iter().collect();

        let results_futures = future::join_all(pages.into_iter().
            map(|page|{
                return self.client.get(format!("https://api.github.com/search/repositories?q=language:{language}&sort=stars&order=desc&page={page_num}&per_page=100", language = lang, page_num = page))
                    .header("Authorization",  format!("token {token_string}", token_string = self.token))
                    .header("User-Agent", "Request")
                    .header("Accept", "application/vnd.github.v3+json")
                    .send()
            })
        );

        let results = results_futures.await;

        let response_body_texts_futures = future::join_all(results.into_iter().map(|result| {
            let res = match result {
                Ok(response_ok) => match response_ok.error_for_status() {
                    Ok(_res) => _res,
                    Err(err) => {
                        panic!("{}", err);
                    }
                },
                Err(err) => panic!("{:#?}", err),
            };

            res.text()
        }));

        let response_body_texts = executor::block_on(response_body_texts_futures);

        let mut responses_parsed: Vec<RepositoriesInfo> = response_body_texts
            .into_iter()
            .map(|resp| {
                let response = resp.unwrap();
                let response_parsed = parse_response(response).unwrap();
                response_parsed
            })
            .collect();

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
        &self,
        repo_info: SingleRepoInfo,
    ) -> Result<(SingleRepoInfo, Vec<RepoContributorsInfo>), reqwest::Error> {
        let num_of_pages =
            get_num_of_pages_for_contributors_on_main_branch(&repo_info, &self.client, &self.token)
                .await?;

        let pages: Vec<u32> = (1..num_of_pages + 1).into_iter().collect();

        unimplemented!();

        let commit_infos_results = future::join_all(pages.iter()
            .map(|item| {
                self.client.get(format!("https://api.github.com/repos/{owner}/{repo_name}/contributors?q=anon=false&page={page_num}&per_page=100&anon=true", owner = repo_info.owner.login, repo_name = repo_info.name, page_num = item))
                    .header("Authorization",  format!("token {token_string}", token_string = self.token))
                    .header("User-Agent", "Request")
                    .header("Accept", "application/vnd.github.v3+json")
                    .send()
                    .then(|response|{
                        response.map(|result|result.error_for_status().unwrap()).unwrap().text()
                    })
                    .then(|resp_text|{
                        async move {
                            let info = serde_json::from_str::<Vec<RepoContributorsInfo>>(&resp_text.unwrap()).unwrap();
                            let info = info;
                            info
                        }
                    })
            }));

        // let commit_infos: Vec<RepoContributorsInfo> = commit_infos_results
        //     .into_iter()
        //     .flat_map(|res| res)
        //     .collect();
        
        //return Ok((repo_info, commit_infos));
    }
}

use regex::Regex;

//This seems stupid, but could not find a better way
//If there is a reason not to hire me, this is it
async fn get_num_of_pages_for_contributors_on_main_branch(
    single_repo_info: &SingleRepoInfo,
    client: &reqwest::Client,
    token: &String,
) -> Result<u32, reqwest::Error> {
    let result =  client.get(format!("https://api.github.com/repos/{owner}/{repo_name}/contributors?q=page=1&per_page=100&anon=true", owner = single_repo_info.owner.login, repo_name = single_repo_info.name))
        .header("Authorization",  format!("token {token_string}", token_string = token))
        .header("User-Agent", "Request")
        .header("Accept", "application/vnd.github.v3+json")
        .send().await?;

    let result = result.error_for_status()?;

    if result.headers().contains_key("link") == false {
        return Ok(1);
    }

    let link = &result.headers()["link"];

    let regex = Regex::new(r"(?m)page=(\d*)").unwrap();

    let result = regex.captures_iter(link.to_str().unwrap());

    let num_of_pages = result
        .last()
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();

    return Ok(num_of_pages);
}
