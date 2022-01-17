use clap::Parser;
use serde::{Deserialize, Serialize};

use futures::executor;
use futures::future;

use indicatif::ProgressBar;

use std::env;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct BusFactoratorArgs {
    /// Name of the language
    #[clap(long, default_value = "rust")]
    language: String,

    /// Ammount of repos to evaluate
    #[clap(long, default_value_t = 950)]
    project_count: u64,
}

use reqwest::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = BusFactoratorArgs::parse();

    let token = match env::var("TOKEN") {
        Ok(val) => {
            val
        },
        Err(e) => panic!("couldn't interpret TOKEN: {}", e),
    };

    let token_string = token.to_string();

    let client = reqwest::Client::new();

    let response = get_repositories_info(&token_string, args.language, args.project_count, &client).await;

    let parsed_repos_info = match response {
        Ok(repos_info) => repos_info,
        Err(err) => {
            panic!("Unable to retrieve or parse response\nErr string: {:#?}", err);
        }
    };

    let single_repos_infos_future = future::join_all(parsed_repos_info.into_iter()
        .map(|repo_info| {
            get_single_repo_info(repo_info, &token_string, &client)
        }));

    Ok(())
}

async fn get_single_repo_info(repo_info : SingleRepoInfo, token : &String, client : &reqwest::Client) -> Result<(SingleRepoInfo, Vec<SingleCommitInfo>)>
{
    let num_of_commits = get_num_of_commits_on_main_branch(&repo_info, &client, token);

    println!("number of commits on {repo_name} is {num}", repo_name = repo_info.name, num = num_of_commits);

    let number_of_pages = num_of_commits / 100;

    let pages : Vec<u64> = (1..number_of_pages).into_iter().collect();

    let futures = future::join_all(pages.iter()
        .map(|item| {
            client.get(format!("https://api.github.com/repos/{owner}/{repo_name}/commits?q=per_page=100&page={page_num}", owner = repo_info.owner.login, repo_name = repo_info.name, page_num = item))
                .header("Authorization",  format!("token {token_string}", token_string = token))
                .header("User-Agent", "Request")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
        })).await;
    
    let commit_info_texts = future::join_all(futures.into_iter()
            .map(|response| 
            {
                let resp = response.unwrap();
                resp.text()
            })).await;

    let commits_parsed : Vec<SingleCommitInfo> = commit_info_texts.into_iter()
        .flat_map(|res| {
            parse_commit_info(res.unwrap()).unwrap()
        }).collect();
    
    return Ok((repo_info, commits_parsed));
}

async fn get_repositories_info(token : &String, lang : String, num_of_repos : u64, client : &reqwest::Client) -> Result<Vec<SingleRepoInfo>>
{
    let required_calls = num_of_repos / 100;
    let last_page_num_of_repos = num_of_repos % 100;
    let bar = ProgressBar::new(required_calls);
    

    let pages : Vec<u64> = (1..required_calls).into_iter().collect();

    let results_futures = future::join_all(pages.into_iter().
        map(|page|{
            return client.get(format!("https://api.github.com/search/repositories?q=language:{language}&sort=stars&order=desc&per_page=100&page={page_num}", language = lang, page_num = page))
                .header("Authorization",  format!("token {token_string}", token_string = token))
                .header("User-Agent", "Request")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
        })
    );

    let results = executor::block_on(results_futures);

    let response_body_texts_futures = future::join_all(results.into_iter()
        .map(|result| result.unwrap().text())
    ); 

    let response_body_texts = executor::block_on(response_body_texts_futures);

    let mut responses_parsed : Vec<RepositoriesInfo> = response_body_texts.into_iter()
        .map(|resp| {
                let response_parsed = parse_response(resp.unwrap())
                    .unwrap();
                bar.inc(1);
                response_parsed
            }
        )
        .collect();
    
    bar.finish();

    responses_parsed.last_mut().unwrap().items.truncate(last_page_num_of_repos.try_into().unwrap());

    let single_repo_vec : Vec<SingleRepoInfo> = responses_parsed.into_iter().flat_map(|element| element.items).collect();

    return Ok(single_repo_vec);
}

#[derive(Serialize, Deserialize, Debug)]
struct SingleAuthorDetails
{
    name : String,
    email : String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SingleCommitDetails
{
    url : String,
    author : SingleAuthorDetails,
}

#[derive(Serialize, Deserialize, Debug)]
struct SingleCommitInfo 
{
    commit : SingleCommitDetails,
}

fn parse_commit_info(req : String) -> serde_json::Result<Vec<SingleCommitInfo>>
{
    let repositories_info : Vec<SingleCommitInfo> = serde_json::from_str(&req)?;
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
    num_of_commits : u64,
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
fn get_num_of_commits_on_main_branch(single_repo_info : &SingleRepoInfo, client : &reqwest::Client, token : &String) -> u64 
{
    let result_future =  client.get(format!("https://api.github.com/repos/{owner}/{repo_name}/commits?per_page=1", owner = single_repo_info.owner.login, repo_name = single_repo_info.name))
        .header("Authorization",  format!("token {token_string}", token_string = token))
        .header("User-Agent", "Request")
        .header("Accept", "application/vnd.github.v3+json")
        .send();

    let result = executor::block_on(result_future).unwrap();

    let link = &result.headers()["link"];
    
    let regex = Regex::new(r"(?m)page=(\d*)").unwrap();

    let result = regex.captures_iter(link.to_str().unwrap());

    let num_of_commits = result.last().unwrap().get(1).unwrap().as_str().parse().unwrap();

    return num_of_commits;
}
