use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tonic::{Request, Status, metadata::MetadataValue};
type WorkerMac = Hmac<Sha256>;
const WORKER_SCOPE: &[u8] = b"xovyra.asset-storage.deletion-worker.v1";
const WORKER_HEADER: &str = "x-xovyra-asset-worker";
pub fn attach<T>(request: &mut Request<T>, secret: &str) -> Result<(), Status> {
    let signature = signature(secret)?;
    let metadata = MetadataValue::try_from(signature.as_str())
        .map_err(|_| Status::internal("The internal worker credential is invalid"))?;
    request.metadata_mut().insert(WORKER_HEADER, metadata);
    Ok(())
}
pub fn verify<T>(request: &Request<T>, secret: &str) -> Result<(), Status> {
    let supplied = request
        .metadata()
        .get(WORKER_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| URL_SAFE_NO_PAD.decode(value).ok())
        .ok_or_else(|| Status::permission_denied("Asset worker authorization is required"))?;
    let mut verifier = WorkerMac::new_from_slice(secret.as_bytes())
        .map_err(|_| Status::internal("The internal worker credential is invalid"))?;
    verifier.update(WORKER_SCOPE);
    verifier
        .verify_slice(&supplied)
        .map_err(|_| Status::permission_denied("Asset worker authorization is invalid"))?;
    Ok(())
}
fn signature(secret: &str) -> Result<String, Status> {
    signature_bytes(secret).map(|value| URL_SAFE_NO_PAD.encode(value))
}
fn signature_bytes(secret: &str) -> Result<Vec<u8>, Status> {
    let mut signer = WorkerMac::new_from_slice(secret.as_bytes())
        .map_err(|_| Status::internal("The internal worker credential is invalid"))?;
    signer.update(WORKER_SCOPE);
    Ok(signer.finalize().into_bytes().to_vec())
}
#[cfg(test)]
mod tests {
    use super::{attach, verify};
    use tonic::{Request, Status};
    #[test]
    fn worker_signatures_are_scope_bound_and_reject_missing_or_wrong_secret() -> Result<(), Status>
    {
        let mut request = Request::new(());
        assert!(verify(&request, "a-secret-with-at-least-thirty-two-characters").is_err());
        attach(&mut request, "a-secret-with-at-least-thirty-two-characters")?;
        assert!(verify(&request, "a-secret-with-at-least-thirty-two-characters").is_ok());
        assert!(
            verify(
                &request,
                "a-different-secret-with-at-least-thirty-two-characters"
            )
            .is_err()
        );
        Ok(())
    }
}
