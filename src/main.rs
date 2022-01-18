use clap::Parser;
use serde::{Deserialize, Serialize};

use futures::executor;
use futures::future;
use futures::future::FutureExt;

use indicatif::ProgressBar;
use indicatif::ProgressStyle;

use std::env;

use reqwest::Result;

use tokio::task;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct BusFactoratorArgs {
    /// Name of the language
    #[clap(long, default_value = "rust")]
    language: String,

    /// Ammount of repos to evaluate
    #[clap(long, default_value_t = 50)]
    project_count: u64,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 12)]
async fn main() -> Result<()> {
    let args = BusFactoratorArgs::parse();

    let token = match env::var("TOKEN") {
        Ok(val) => {
            val
        },
        Err(e) => panic!("couldn't find env variable TOKEN: {}", e),
    };

    let token_string = token.to_string();

    let client = reqwest::Client::new();

    let response = get_repositories_info(&token_string, args.language, args.project_count, client.clone()).await;

    let parsed_repos_info = match response {
        Ok(repos_info) => repos_info,
        Err(err) => {
            panic!("Unable to retrieve or parse repositories info: {:#?}", err);
        }
    };

    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{spinner} {wide_msg}");

    let bar = ProgressBar::new(parsed_repos_info.len().try_into().unwrap());

    bar.set_style(spinner_style);

    let single_repos_infos_results : Vec<(SingleRepoInfo, Vec<RepoContributorsInfo>)> = future::join_all(parsed_repos_info.into_iter()
        .map(|repo_info| {
            let task_client = client.clone();
            let task_token = token.clone();
            task::spawn( async move {
                get_single_repo_info(repo_info, task_token, task_client).await
            })
            .then(|fut| {
                let future_unwrapped = fut.expect("failed to retrieve future from tokio task, something went wrong big time");
                
                let mut repo_info = future_unwrapped.unwrap();

                async move {
                    let commits_num = repo_info.1.iter().fold(0, |acc, x| acc + x.contributions);
                    repo_info.0.num_of_commits = commits_num;

                    repo_info
                }
            })
            .then(|repo_info| {
                let bar_copy = bar.clone();

                async move {
                    bar_copy.set_message(format!("Processed data for repository {}", repo_info.0.full_name));
                    bar_copy.inc(1);
                    repo_info
                }
            })
        })).await;

    let single_repos_infos_results_parsed : Vec<(SingleRepoInfo, Vec<RepoContributorsInfo>)> = single_repos_infos_results.into_iter()
        .map(|data| {
            let mut repo_info = data.0;
            let most_commits = data.1.first().expect("list of contributors to repository is empty");
            repo_info.bus_factor = most_commits.contributions as f32 / repo_info.num_of_commits as f32;

            (repo_info, data.1)
        })
        .filter(|data| data.0.bus_factor > 0.75f32).collect();

    bar.finish_with_message("data collection finished");

    single_repos_infos_results_parsed.into_iter()
        .for_each(|data|{
            println!("project: {name} user: {user} percentage:{bus_factor}", name = data.0.full_name, user = data.1.first().unwrap().login, bus_factor = data.0.bus_factor);
        });
    Ok(())
}

async fn get_single_repo_info(repo_info : SingleRepoInfo, token : String, client : reqwest::Client) -> Result<(SingleRepoInfo, Vec<RepoContributorsInfo>)>
{
    let num_of_pages = get_num_of_pages_for_contributors_on_main_branch(&repo_info, &client, &token).await?;

    let pages : Vec<u64> = (1..num_of_pages + 1).into_iter().collect();

    let commit_infos_results = future::join_all(pages.iter()
        .map(|item| {
            client.get(format!("https://api.github.com/repos/{owner}/{repo_name}/contributors?q=anon=false&page={page_num}&per_page=100&anon=true", owner = repo_info.owner.login, repo_name = repo_info.name, page_num = item))
                .header("Authorization",  format!("token {token_string}", token_string = token))
                .header("User-Agent", "Request")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
                .then(|response|{
                    response.map(|result|result.error_for_status().unwrap()).unwrap().text()
                })
                .then(|resp_text|{
                    async move {
                        let info = parse_contributors_info(resp_text.unwrap()).await;
                        let info = info.unwrap();
                        info
                    }
                })
        })).await;

    let commit_infos : Vec<RepoContributorsInfo> = commit_infos_results.into_iter()
        .flat_map(|res| {
            res
        }).collect();

    return Ok((repo_info, commit_infos));
}

async fn get_repositories_info(token : &String, lang : String, num_of_repos : u64, client : reqwest::Client) -> Result<Vec<SingleRepoInfo>>
{
    let required_calls = (num_of_repos / 100) + 1;
    let last_page_num_of_repos = num_of_repos % 100;
    let bar = ProgressBar::new(required_calls);
    

    let pages : Vec<u64> = (1..required_calls + 1).into_iter().collect();

    let results_futures = future::join_all(pages.into_iter().
        map(|page|{
            return client.get(format!("https://api.github.com/search/repositories?q=language:{language}&sort=stars&order=desc&page={page_num}&per_page=100", language = lang, page_num = page))
                .header("Authorization",  format!("token {token_string}", token_string = token))
                .header("User-Agent", "Request")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
        })
    );

    let results = results_futures.await;

    let response_body_texts_futures = future::join_all(results.into_iter()
        .map(|result| {
            let res = match result {
                Ok(response_ok) => {
                    match response_ok.error_for_status() {
                        Ok(_res) => _res,
                        Err(err) => {
                               panic!("{}", err);
                        }
                    }
                },
                Err(err) => panic!("{:#?}", err),
            };
            

            res.text()
        })
    ); 

    let response_body_texts = executor::block_on(response_body_texts_futures);

    let mut responses_parsed : Vec<RepositoriesInfo> = response_body_texts.into_iter()
        .map(|resp| {
                let response = resp.unwrap();
                let response_parsed = parse_response(response)
                    .unwrap();
                bar.inc(1);
                response_parsed
            }
        )
        .collect();
    

    responses_parsed.last_mut().unwrap().items.truncate(last_page_num_of_repos.try_into().unwrap());

    let single_repo_vec : Vec<SingleRepoInfo> = responses_parsed.into_iter().flat_map(|element| element.items).collect();

    return Ok(single_repo_vec);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RepoContributorsInfo {
    #[serde(alias = "name")]
    login : String,
    contributions : u32,
}

async fn parse_contributors_info(req : String) -> serde_json::Result<Vec<RepoContributorsInfo>>
{
    let repositories_info : Vec<RepoContributorsInfo> = serde_json::from_str(&req)?;
    Ok(repositories_info)
}

#[derive(Serialize, Deserialize, Debug)]
struct OwnerInfo 
{
    login : String
}

#[derive(Serialize, Deserialize, Debug)]
struct SingleRepoInfo 
{
    id : u64,
    name : String,
    full_name : String,
    stargazers_count : u64,
    commits_url : String,
    owner : OwnerInfo,
    size : u64,
    #[serde(default)]
    num_of_commits : u32,
    #[serde(default)]
    bus_factor : f32,
}

#[derive(Serialize, Deserialize, Debug)]
struct RepositoriesInfo 
{
    items : Vec<SingleRepoInfo>,
    total_count : u64,
    incomplete_results : bool
}

fn parse_response(req : String) -> serde_json::Result<RepositoriesInfo>
{
    let repositories_info = serde_json::from_str::<RepositoriesInfo>(&req)?;
    Ok(repositories_info)
}

use regex::Regex;

//This seems stupid, but could not find a better way
//If there is a reason not to hire me, this is it
async fn get_num_of_pages_for_contributors_on_main_branch(single_repo_info : &SingleRepoInfo, client : &reqwest::Client, token : &String) -> Result<u64> 
{
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

    let num_of_pages = result.last().unwrap().get(1).unwrap().as_str().parse().unwrap();

    return Ok(num_of_pages);
}
