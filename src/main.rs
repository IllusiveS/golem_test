use clap::Parser;

use std::env;

use anyhow::Result;

mod github_provider;

use tokio::task;

use crate::github_provider::GithubProvider;
use github_provider::APIGithubProvider;

use crate::github_provider::RepoContributorsInfo;
use crate::github_provider::SingleRepoInfo;

use log::warn;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct BusFactoratorArgs {
    /// Name of the language
    #[clap(long, default_value = "rust")]
    language: String,

    /// Ammount of repos to evaluate
    #[clap(long, default_value_t = 50)]
    project_count: u32,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = BusFactoratorArgs::parse();

    let token = match env::var("TOKEN") {
        Ok(val) => val,
        Err(e) => panic!("couldn't find env variable TOKEN: {}", e),
    };

    let client = APIGithubProvider::new(token.clone());

    let response: Result<Vec<_>> = client
        .gather_repositories_info(args.language, args.project_count)
        .await;

    let parsed_repos_info = match response {
        Ok(repos_info) => repos_info,
        Err(err) => {
            panic!("Unable to retrieve or parse repositories info: {:#?}", err);
        }
    };

    let single_repo_info_futures = parsed_repos_info
        .into_iter()
        .map(|repo_info| {
            let client_ref = client.clone();
            task::spawn(async move { client_ref.gather_single_repository_info(repo_info) })
        })
        .collect::<Vec<_>>();

    let mut parsed_single_repos_info: Vec<_> = vec![];

    for obj in single_repo_info_futures {
        let result = obj
            .await
            .expect("Tokio join handle return failed, something went unexpectedly bad")
            .await;

        match result {
            Ok(result) => parsed_single_repos_info.push(result),
            Err(err) => warn!(
                "Unable to retrieve information about repository\n{err}",
                err = err
            ),
        }
    }

    let parsed_single_repos_info: Vec<(SingleRepoInfo, Vec<RepoContributorsInfo>)> =
        parsed_single_repos_info
            .into_iter()
            .map(|data| {
                let mut repo_info = data.0;
                let most_commits = data
                    .1
                    .first()
                    .expect("list of contributors to repository is empty");
                repo_info.bus_factor =
                    most_commits.contributions as f32 / repo_info.num_of_commits as f32;

                (repo_info, data.1)
            })
            .filter(|data| data.0.bus_factor > 0.75f32)
            .collect();

    parsed_single_repos_info.into_iter().for_each(|data| {
        println!(
            "project: {name} user: {user} percentage:{bus_factor}",
            name = data.0.full_name,
            user = data.1.first().unwrap().login,
            bus_factor = data.0.bus_factor
        );
    });
    Ok(())
}
