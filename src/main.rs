use clap::Parser;

// use futures::future;
// use futures::future::FutureExt;

// use indicatif::ProgressBar;
// use indicatif::ProgressStyle;

// use crate::github_provider::RepoContributorsInfo;
// use crate::github_provider::SingleRepoInfo;

// use tokio::task;

use std::env;

use anyhow::Result;
mod github_provider;

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
        Ok(val) => val,
        Err(e) => panic!("couldn't find env variable TOKEN: {}", e),
    };

    let token_string = token.to_string();

    let client = reqwest::Client::new();

    unimplemented!();

    // let response = get_repositories_info(
    //     &token_string,
    //     args.language,
    //     args.project_count,
    //     client.clone(),
    // )
    // .await;

    // let parsed_repos_info = match response {
    //     Ok(repos_info) => repos_info,
    //     Err(err) => {
    //         panic!("Unable to retrieve or parse repositories info: {:#?}", err);
    //     }
    // };

    // let spinner_style = ProgressStyle::default_spinner()
    //     .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
    //     .template("{spinner} {wide_msg}");

    // let bar = ProgressBar::new(parsed_repos_info.len().try_into().unwrap());

    // bar.set_style(spinner_style);

    // let single_repos_infos_results: Vec<(SingleRepoInfo, Vec<RepoContributorsInfo>)> =
    //     future::join_all(parsed_repos_info.into_iter().map(|repo_info| {
    //         let task_client = client.clone();
    //         let task_token = token.clone();
    //         task::spawn(
    //             async move { get_single_repo_info(repo_info, task_token, task_client).await },
    //         )
    //         .then(|fut| {
    //             let future_unwrapped = fut.expect(
    //                 "failed to retrieve future from tokio task, something went wrong big time",
    //             );

    //             let mut repo_info = future_unwrapped.unwrap();

    //             async move {
    //                 let commits_num = repo_info.1.iter().fold(0, |acc, x| acc + x.contributions);
    //                 repo_info.0.num_of_commits = commits_num;

    //                 repo_info
    //             }
    //         })
    //         .then(|repo_info| {
    //             let bar_copy = bar.clone();

    //             async move {
    //                 bar_copy.set_message(format!(
    //                     "Processed data for repository {}",
    //                     repo_info.0.full_name
    //                 ));
    //                 bar_copy.inc(1);
    //                 repo_info
    //             }
    //         })
    //     }))
    //     .await;

    // let single_repos_infos_results_parsed: Vec<(SingleRepoInfo, Vec<RepoContributorsInfo>)> =
    //     single_repos_infos_results
    //         .into_iter()
    //         .map(|data| {
    //             let mut repo_info = data.0;
    //             let most_commits = data
    //                 .1
    //                 .first()
    //                 .expect("list of contributors to repository is empty");
    //             repo_info.bus_factor =
    //                 most_commits.contributions as f32 / repo_info.num_of_commits as f32;

    //             (repo_info, data.1)
    //         })
    //         .filter(|data| data.0.bus_factor > 0.75f32)
    //         .collect();

    // bar.finish_with_message("data collection finished");

    // single_repos_infos_results_parsed
    //     .into_iter()
    //     .for_each(|data| {
    //         println!(
    //             "project: {name} user: {user} percentage:{bus_factor}",
    //             name = data.0.full_name,
    //             user = data.1.first().unwrap().login,
    //             bus_factor = data.0.bus_factor
    //         );
    //     });
    Ok(())
}
