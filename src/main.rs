use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct BusFactoratorArgs {
    /// Name of the language
    #[clap(long, default_value = "rust")]
    language: String,

    /// Ammount of repos to evaluate
    #[clap(long, default_value_t = 1)]
    project_count: u8,
}

use reqwest::Response;
use reqwest::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = BusFactoratorArgs::parse();

    let response = single_request().await?;

    print!("{:#?}", response);

    Ok(())
}


async fn single_request() -> Result<Response>
{
    let client = reqwest::Client::new();

    let request_params = [("sort", "stars")];
    let result =  client.get("https://api.github.com/search/repositories")
        .form(&request_params)
        .send()
        .await?;

    return Ok(result);
}