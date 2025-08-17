use eyre::{Context, Result, eyre};

/// Enhanced CRF parser that enforces strictly descending values
/// Supported formats:
/// - Single values (35) → [35]
/// - Comma-separated lists (35,27,21) → [35, 27, 21]
/// - Backward ranges (36..21) → [36, 35, ..., 21]
/// - Stepped backward ranges (36..21:3) → [36, 33, 30, ..., 21]
pub fn crf_parser(s: &str) -> Result<Vec<u8>> {
    // Parse the raw values first
    let values = parse_raw_crf_values(s)?;

    // Validate descending order
    validate_descending(&values).wrap_err_with(|| {
        format!("CRF values must be in strictly descending order (got {values:?})")
    })?;

    Ok(values)
}

/// Core parsing logic
pub fn parse_raw_crf_values(s: &str) -> Result<Vec<u8>> {
    const CRF_RANGE: std::ops::RangeInclusive<u8> = 1..=70;

    let validate_crf = |value: u8| {
        if !CRF_RANGE.contains(&value) {
            Err(eyre!(
                "CRF must be between {}-{} (got {})",
                CRF_RANGE.start(),
                CRF_RANGE.end(),
                value
            ))
        } else {
            Ok(value)
        }
    };

    // Handle stepped ranges (36..21:3)
    if let Some((range_part, step)) = s.split_once(':')
        && let Some((start, end)) = range_part.split_once("..")
    {
        let (start, end, step) = (
            start
                .parse()
                .wrap_err_with(|| format!("Invalid range start: '{start}'"))?,
            end.parse()
                .wrap_err_with(|| format!("Invalid range end: '{end}'"))?,
            step.parse()
                .wrap_err_with(|| format!("Invalid step value: '{step}'"))?,
        );

        if start < end {
            return Err(eyre!(
                "Backward range requires start >= end (got {start}..{end})"
            ));
        }
        if step == 0 {
            return Err(eyre!("Step value must be positive"));
        }

        let mut values = Vec::new();
        let mut current = start;
        while current >= end {
            values.push(validate_crf(current)?);
            current = current.saturating_sub(step);
        }
        return Ok(values);
    }

    // Handle simple ranges (36..21)
    if let Some((start, end)) = s.split_once("..") {
        let (start, end) = (
            start
                .parse()
                .wrap_err_with(|| format!("Invalid range start: '{start}'"))?,
            end.parse()
                .wrap_err_with(|| format!("Invalid range end: '{end}'"))?,
        );

        if start < end {
            return Err(eyre!(
                "Backward range requires start >= end (got {start}..{end})"
            ));
        }

        return (end..=start).rev().map(validate_crf).collect();
    }

    // Handle comma-separated or single value
    s.split(',')
        .map(|part| {
            part.trim()
                .parse()
                .wrap_err_with(|| format!("Invalid CRF value: '{}'", part.trim()))
                .and_then(validate_crf)
        })
        .collect()
}

/// Validate strict descending order
pub fn validate_descending(values: &[u8]) -> Result<()> {
    if values.windows(2).any(|pair| pair[0] <= pair[1]) {
        Err(eyre!("Sequence contains non-descending values"))
    } else {
        Ok(())
    }
}
