use flate2::write::GzEncoder;
use flate2::Compression;
use git2 as git;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use s3::{creds::Credentials, Bucket, Region};
use serde::{Deserialize, Serialize};
use serde_json as JSON;

mod config;
use config::*;
lazy_static::lazy_static! {
    static ref CREDENTIALS: Credentials =
        Credentials::new(Some(&*S3_KEY), Some(&*S3_SECRET), None, None, None).unwrap();
    static ref REGION: Region = Region::Custom {
        region: S3_REGION.clone(),
        endpoint: S3_ENDPOINT.clone(),
    };
}

#[derive(Serialize, Deserialize, Debug)]
struct RepositoryOwner {
    login: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Repository {
    name: String,
    full_name: String,
    private: bool,
    fork: bool,
    default_branch: String,
    owner: RepositoryOwner,
}

async fn get_repositories() -> anyhow::Result<Vec<Repository>> {
    let body = reqwest::Client::new()
        .get("https://api.github.com/user/repos?per_page=100&sort=pushed")
        .header(AUTHORIZATION, format!("token {}", &*GH_TOKEN))
        .header(USER_AGENT, "rust")
        .send()
        .await?
        .text()
        .await?;

    Ok(JSON::from_str(&body).expect("Failed to parse JSON"))
}

fn dir_to_tar(path: &str, src_path: &str) -> std::io::Result<Vec<u8>> {
    let enc = GzEncoder::new(vec![], Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(path, src_path)?;
    tar.into_inner()?.finish()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let repos = get_repositories()
        .await
        .expect("Failed to fetch repositories.");

    let bucket =
        Bucket::new(&*S3_BUCKET_NAME, REGION.clone(), CREDENTIALS.clone())?.with_path_style();

    for repo in repos {
        if repo.fork || repo.owner.login != *GITHUB_ACTOR {
            continue;
        }

        let s3_path = format!("{}.tar.gz", repo.full_name);
        let clone_path = format!("clones/{}", repo.full_name);

        std::fs::remove_dir_all(&clone_path).ok();

        git::Repository::clone(
            &format!(
                "https://{}:{}@github.com/{}.git",
                &*GITHUB_ACTOR, &*GH_TOKEN, repo.full_name
            ),
            &clone_path,
        )
        .expect("can't clone");

        let buf = dir_to_tar(&repo.name, &clone_path)?;

        bucket
            .put_object(s3_path, &buf)
            .await
            .expect("Failed to upload to s3");
    }

    println!("Finished.");

    Ok(())
}
