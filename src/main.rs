use clap::Parser;

use std::env;
use std::sync::Arc;

use anyhow::Result;

mod github_provider;

use crate::github_provider::GithubProvider;
use crate::github_provider::APIGithubProvider;

use log::{info, warn, LevelFilter};

use simple_logger::SimpleLogger;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct BusFactoratorArgs {
    /// Name of the language
    #[clap(long, default_value = "rust")]
    language: String,

    /// Ammount of repos to evaluate
    #[clap(long, default_value_t = 50)]
    project_count: u32,

    /// Options are Off,Error,Warn,Info,Debug,Trace,
    #[clap(short, long, default_value_t = log::LevelFilter::Info)]
    log_level: LevelFilter,
}

fn init_logger(args : &BusFactoratorArgs) {
    if args.log_level != log::LevelFilter::Off {
        let logger = SimpleLogger::new();

        let logger = logger.with_level(args.log_level);
    
        logger.init().expect("Failed to initialize logger");
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = BusFactoratorArgs::parse();

    init_logger(&args);

    //Czy akceptowalne tu byłoby token = env::var("TOKEN")?
    //I tak leci panic! na program
    let token = match env::var("TOKEN") {
        Ok(val) => val,
        Err(e) => panic!("couldn't find env variable TOKEN: {}", e),
    };
    
    let client = Arc::new(APIGithubProvider::new(token.clone()));
    info!("Client established");

    let response: Result<Vec<_>> = client
        .clone()
        .gather_repositories_info(args.language, args.project_count)
        .await;

    let parsed_repos_info = match response {
        Ok(repos_info) => repos_info,
        Err(err) => {
            panic!("Unable to retrieve or parse repositories info: {:#?}", err);
        }
    };

    info!("Gathered repositories");

    let single_repo_info_futures = parsed_repos_info
        .into_iter()
        .map(|repo_info| {
            tokio::task::spawn(client.clone().gather_single_repository_info(repo_info))
        })
        .collect::<Vec<_>>();

    let mut parsed_single_repos_info: Vec<_> = Vec::with_capacity(single_repo_info_futures.len());

    for obj in single_repo_info_futures {
        let result = obj.await?;

        match result {
            Ok(result) => parsed_single_repos_info.push(result),
            Err(err) => warn!(
                "Unable to retrieve information about repository\n{err}",
                err = err
            ),
        }
    }

    info!("Gathered individual repo infos");

    let parsed_single_repos_info = parsed_single_repos_info
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
        .filter(|data| data.0.bus_factor > 0.75f32);

    parsed_single_repos_info.for_each(|data| {
        println!(
            "project: {name} user: {user} percentage:{bus_factor}",
            name = data.0.full_name,
            user = data.1.first().unwrap().login,
            bus_factor = data.0.bus_factor
        );
    });

    Ok(())
}
