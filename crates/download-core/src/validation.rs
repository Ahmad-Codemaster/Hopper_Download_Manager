use crate::model::{EngineError, JobSpec};
use url::Url;

const MAX_HEADERS: usize = 32;
const MAX_HEADER_LENGTH: usize = 4096;

pub fn validate_job_spec(spec: &JobSpec) -> Result<(), EngineError> {
    match spec.source.scheme() {
        "http" | "https" => {}
        _ => return Err(EngineError::UnsupportedScheme),
    }

    if spec.destination.is_absolute()
        || spec.destination.components().any(|component| {
            matches!(component, std::path::Component::ParentDir)
        })
        || spec.destination.as_os_str().is_empty()
    {
        return Err(EngineError::UnsafeDestination);
    }

    if spec.request_context.headers.len() > MAX_HEADERS
        || spec.request_context.headers.iter().any(|(name, value)| {
            name.len() > MAX_HEADER_LENGTH || value.len() > MAX_HEADER_LENGTH
        })
    {
        return Err(EngineError::RequestContextTooLarge);
    }

    Ok(())
}

pub fn redact_url(url: &Url) -> String {
    let mut redacted = url.clone();
    if redacted.query().is_some() {
        let query = redacted
            .query_pairs()
            .map(|(key, _)| format!("{key}=[REDACTED]"))
            .collect::<Vec<_>>()
            .join("&");
        redacted.set_query(Some(&query));
    }
    redacted.to_string()
}
