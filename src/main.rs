use flate2::write::GzEncoder;
use flate2::Compression;
use git2 as git;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use s3::{creds::Credentials, Bucket, Region};
use serde::{Deserialize, Serialize};
use serde_json as JSON;
use std::collections::HashMap;

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

#[derive(Serialize, Deserialize, Debug)]
struct GistFile {
    filename: String,
    language: Option<String>,
    raw_url: String,
    size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct Gist {
    id: String,
    public: bool,
    description: String,
    created_at: String,
    updated_at: String,
    files: HashMap<String, GistFile>,
}

async fn file_content(file: &GistFile) -> anyhow::Result<String> {
    let content = reqwest::Client::new()
        .get(&file.raw_url)
        .send()
        .await?
        .text()
        .await?;

    Ok(content)
}

async fn get_repositories() -> anyhow::Result<Vec<Repository>> {
    let body = reqwest::Client::new()
        .get("https://api.github.com/user/repos?per_page=100&sort=pushed")
        .header(AUTHORIZATION, format!("token {}", &*GH_TOKEN))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "rust")
        .send()
        .await?
        .text()
        .await?;

    Ok(JSON::from_str(&body).expect("Failed to parse JSON"))
}

async fn get_gists() -> anyhow::Result<Vec<Gist>> {
    let body = reqwest::Client::new()
        .get("https://api.github.com/gists?per_page=100")
        .header(AUTHORIZATION, format!("token {}", &*GH_TOKEN))
        .header(ACCEPT, "application/vnd.github+json")
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

    let gists = get_gists().await.expect("Failed to fetch private gists.");

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

    for gist in gists {
        for (filename, file) in gist.files {
            let path = format!(
                "gists/{}/{}-{}",
                if gist.public { "public" } else { "private" },
                gist.id[0..5].to_string(),
                filename
            );
            bucket
                .put_object(path, file_content(&file).await?.as_bytes())
                .await
                .expect("Failed to upload to s3");
        }
    }

    println!("Finished.");

    Ok(())
}
