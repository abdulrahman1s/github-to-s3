use crate::config::*;
use s3::{creds::Credentials, error::S3Error, Bucket, Region};

lazy_static::lazy_static! {
    static ref CREDENTIALS: Credentials =
        Credentials::new(Some(&*S3_KEY), Some(&*S3_SECRET), None, None, None).unwrap();
    static ref REGION: Region = Region::Custom {
        region: S3_REGION.clone(),
        endpoint: S3_ENDPOINT.clone(),
    };
}

pub fn bucket() -> Bucket {
    Bucket::new(&*S3_BUCKET_NAME, REGION.clone(), CREDENTIALS.clone())
        .unwrap()
        .with_path_style()
}

pub async fn put(path: &str, content: &[u8]) -> Result<(), S3Error> {
    bucket().put_object(path, content).await?;
    Ok(())
}

