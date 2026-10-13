use crate::model::EngineError;

pub fn validate_content_range(
    value: &str,
    expected_start: u64,
    expected_end: u64,
    expected_total: u64,
) -> Result<(), EngineError> {
    let (unit, range) = value
        .split_once(' ')
        .ok_or_else(|| EngineError::InvalidJob("malformed Content-Range".into()))?;
    if unit != "bytes" {
        return Err(EngineError::InvalidJob("unsupported Content-Range unit".into()));
    }
    let (bounds, total) = range
        .split_once('/')
        .ok_or_else(|| EngineError::InvalidJob("malformed Content-Range".into()))?;
    let (start, end) = bounds
        .split_once('-')
        .ok_or_else(|| EngineError::InvalidJob("malformed Content-Range bounds".into()))?;
    let parsed = (
        start.parse::<u64>(),
        end.parse::<u64>(),
        total.parse::<u64>(),
    );
    if parsed == (Ok(expected_start), Ok(expected_end), Ok(expected_total)) {
        Ok(())
    } else {
        Err(EngineError::InvalidJob("unexpected Content-Range".into()))
    }
}
