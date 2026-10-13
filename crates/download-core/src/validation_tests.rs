#[cfg(test)]
mod tests {
    use super::super::model::{ConflictPolicy, JobSpec, RequestContext};
    use super::super::validation::{redact_url, validate_job_spec};
    use url::Url;

    fn spec(source: &str, destination: &str) -> JobSpec {
        JobSpec {
            source: Url::parse(source).unwrap(),
            destination: destination.into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Reject,
        }
    }

    #[test]
    fn accepts_http_and_https_sources() {
        assert!(validate_job_spec(&spec("https://example.test/file.bin", "file.bin")).is_ok());
        assert!(validate_job_spec(&spec("http://example.test/file.bin", "file.bin")).is_ok());
    }

    #[test]
    fn rejects_non_http_sources_and_traversal() {
        assert!(validate_job_spec(&spec("file:///secret", "file.bin")).is_err());
        assert!(validate_job_spec(&spec("https://example.test/file.bin", "../file.bin")).is_err());
    }

    #[test]
    fn redacts_sensitive_query_values() {
        let url = Url::parse("https://example.test/video?token=secret&part=1").unwrap();
        assert_eq!(redact_url(&url), "https://example.test/video?token=[REDACTED]&part=[REDACTED]");
    }
}
