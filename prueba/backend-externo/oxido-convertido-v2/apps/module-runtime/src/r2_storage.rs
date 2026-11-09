use std::time::Duration;

use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::presigning::PresigningConfig;

pub struct R2Storage {
    client: Client,
    bucket: String,
}

impl R2Storage {
    pub fn from_environment(
        endpoint: Option<&str>,
        bucket: Option<&str>,
        access_key_id: Option<&str>,
        secret_access_key: Option<&str>,
    ) -> Option<Self> {
        let (Some(endpoint), Some(bucket), Some(access_key_id), Some(secret_access_key)) =
            (endpoint, bucket, access_key_id, secret_access_key)
        else {
            return None;
        };
        let credentials = Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "xovyra-cloudflare-r2",
        );
        let configuration = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("auto"))
            .credentials_provider(credentials)
            .endpoint_url(endpoint)
            .force_path_style(true)
            .build();
        Some(Self {
            client: Client::from_conf(configuration),
            bucket: bucket.to_owned(),
        })
    }

    pub async fn presign_upload(&self, key: &str, content_type: &str) -> Result<String, String> {
        let configuration = PresigningConfig::expires_in(Duration::from_secs(300))
            .map_err(|_| "upload URL expiry configuration is invalid".to_owned())?;
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .presigned(configuration)
            .await
            .map_err(|_| "R2 could not create an upload URL".to_owned())?;
        Ok(request.uri().to_string())
    }

    pub async fn presign_download(&self, key: &str) -> Result<String, String> {
        let configuration = PresigningConfig::expires_in(Duration::from_secs(90))
            .map_err(|_| "download URL expiry configuration is invalid".to_owned())?;
        let request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .response_content_disposition("attachment")
            .presigned(configuration)
            .await
            .map_err(|_| "R2 could not create a download URL".to_owned())?;
        Ok(request.uri().to_string())
    }

    pub async fn inspect_object(&self, key: &str) -> Result<(i64, String, String), String> {
        let result = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|_| "R2 could not verify the uploaded object".to_owned())?;
        let size = result.content_length().unwrap_or_default();
        let content_type = result.content_type().unwrap_or_default().to_owned();
        let etag = result.e_tag().unwrap_or_default().to_owned();
        Ok((size, content_type, etag))
    }

    pub async fn promote_object(
        &self,
        source_key: &str,
        destination_key: &str,
    ) -> Result<(), String> {
        self.client
            .copy_object()
            .bucket(&self.bucket)
            .key(destination_key)
            .copy_source(format!("{}/{}", self.bucket, source_key))
            .send()
            .await
            .map_err(|_| "R2 could not promote the uploaded object".to_owned())?;
        Ok(())
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), String> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|_| "R2 could not delete the object".to_owned())?;
        Ok(())
    }
}

#[cfg(all(test, feature = "real-r2-tests"))]
mod real_integration {
    use std::error::Error;

    use super::R2Storage;

    #[tokio::test]
    async fn direct_presigned_upload_head_download_and_delete_use_real_cloudflare_r2()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        xovyra_platform::configuration::load_environment_file()?;
        let endpoint = std::env::var("CLOUDFLARE_R2_ENDPOINT")?;
        let bucket = std::env::var("CLOUDFLARE_R2_BUCKET")?;
        let access_key_id = std::env::var("CLOUDFLARE_R2_ACCESS_KEY_ID")?;
        let secret_access_key = std::env::var("CLOUDFLARE_R2_SECRET_ACCESS_KEY")?;
        let storage = R2Storage::from_environment(
            Some(&endpoint),
            Some(&bucket),
            Some(&access_key_id),
            Some(&secret_access_key),
        )
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Real R2 integration requires all CLOUDFLARE_R2_* credentials",
            )
        })?;
        let workspace_id = uuid::Uuid::new_v4();
        let owner_user_id = uuid::Uuid::new_v4();
        let asset_id = uuid::Uuid::new_v4();
        let key = format!("private/{workspace_id}/{owner_user_id}/{asset_id}");
        let content = format!("R2 transfer integration {asset_id}");
        let verification: Result<(), Box<dyn Error + Send + Sync>> = async {
            let upload_url = storage.presign_upload(&key, "text/plain").await?;
            let upload = reqwest::Client::new()
                .put(upload_url)
                .header(reqwest::header::CONTENT_TYPE, "text/plain")
                .body(content.clone())
                .send()
                .await?;
            let upload_status = upload.status();
            let upload_body = upload.text().await?;
            if !upload_status.is_success() {
                return Err(Box::<dyn Error + Send + Sync>::from(std::io::Error::other(
                    format!("R2 upload returned HTTP {upload_status}: {upload_body}"),
                )));
            }
            let (size, content_type, etag) = storage.inspect_object(&key).await?;
            if size != i64::try_from(content.len())?
                || content_type != "text/plain"
                || etag.is_empty()
            {
                return Err(Box::<dyn Error + Send + Sync>::from(std::io::Error::other(
                    "R2 HEAD did not return the uploaded object metadata",
                )));
            }
            let download_url = storage.presign_download(&key).await?;
            let browser_preflight = reqwest::Client::new()
                .request(reqwest::Method::OPTIONS, &download_url)
                .header(
                    reqwest::header::ORIGIN,
                    "https://admin-xovyra.jnmtortas.com",
                )
                .header(reqwest::header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .send()
                .await?;
            if !browser_preflight.status().is_success()
                || browser_preflight
                    .headers()
                    .get(reqwest::header::ACCESS_CONTROL_ALLOW_ORIGIN)
                    .and_then(|value| value.to_str().ok())
                    != Some("https://admin-xovyra.jnmtortas.com")
            {
                return Err(Box::<dyn Error + Send + Sync>::from(std::io::Error::other(
                    "R2 CORS does not allow browser download from the platform-admin origin",
                )));
            }
            let download = reqwest::Client::new().get(download_url).send().await?;
            if !download.status().is_success() || download.text().await? != content {
                return Err(Box::<dyn Error + Send + Sync>::from(std::io::Error::other(
                    "R2 presigned download returned different object bytes",
                )));
            }
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }
        .await;
        let cleanup = storage.delete_object(&key).await;
        verification?;
        cleanup?;
        Ok(())
    }
}
