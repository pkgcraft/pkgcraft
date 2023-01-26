use anyhow::Result;
use clap::Command;

use crate::Client;
use arcanist::proto::StringRequest;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("list")
        .about("list repos")
        .long_about("List repositories ordered by their priority and then location.")
}

pub async fn run(client: &mut Client) -> Result<()> {
    // TODO: add support for specifying repo types
    let request = tonic::Request::new(StringRequest {
        data: "repos".to_string(),
    });
    let response = client.list_repos(request).await?;
    for repo in response.into_inner().data.iter() {
        println!("{repo}");
    }
    Ok(())
}
