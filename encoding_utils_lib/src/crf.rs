use eyre::{Context, Result, eyre};

/// Enhanced CRF parser that enforces strictly descending values
/// Supported formats:
/// - Single values (35 or 35.5) → [35.0] or [35.5]
/// - Comma-separated lists (35,27.2,21) → [35.0, 27.2, 21.0]
/// - Backward ranges (36..21) → [36.0, 35.0, ..., 21.0]
/// - Stepped backward ranges (36..21:1.5) → [36.0, 34.5, 33.0, ..., 21.0]
pub fn crf_parser(s: &str) -> Result<Vec<f64>> {
    // Parse the raw values first
    let values = parse_raw_crf_values(s)?;

    // Validate descending order
    validate_descending(&values).wrap_err_with(|| {
        format!("CRF values must be in strictly descending order (got {values:?})")
    })?;

    Ok(values)
}

/// Core parsing logic
pub fn parse_raw_crf_values(s: &str) -> Result<Vec<f64>> {
    const CRF_RANGE: std::ops::RangeInclusive<f64> = 1.0..=70.0;

    let validate_crf = |value: f64| {
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

    // Handle stepped ranges (36..21:1.5 or 36.0..21.0:1.5)
    if let Some((range_part, step_str)) = s.split_once(':')
        && let Some((start_str, end_str)) = range_part.split_once("..")
    {
        let start: f64 = start_str
            .parse()
            .wrap_err_with(|| format!("Invalid range start: '{start_str}'"))?;
        let end: f64 = end_str
            .parse()
            .wrap_err_with(|| format!("Invalid range end: '{end_str}'"))?;
        let step: f64 = step_str
            .parse()
            .wrap_err_with(|| format!("Invalid step value: '{step_str}'"))?;

        if start < end {
            return Err(eyre!(
                "Backward range requires start >= end (got {start}..{end})"
            ));
        }
        if step <= 0.0 {
            return Err(eyre!("Step value must be positive"));
        }

        let mut values = Vec::new();
        let mut current = start;
        while current >= end {
            values.push(validate_crf(current)?);
            current -= step;
            // Handle floating point precision issues by rounding
            current = (current * 1000.0).round() / 1000.0;
        }
        return Ok(values);
    }

    // Handle simple ranges (36..21 or 36.0..21.0)
    if let Some((start_str, end_str)) = s.split_once("..") {
        let start: f64 = start_str
            .parse()
            .wrap_err_with(|| format!("Invalid range start: '{start_str}'"))?;
        let end: f64 = end_str
            .parse()
            .wrap_err_with(|| format!("Invalid range end: '{end_str}'"))?;

        if start < end {
            return Err(eyre!(
                "Backward range requires start >= end (got {start}..{end})"
            ));
        }

        // For floating point ranges, we need to generate the sequence manually
        let mut values = Vec::new();
        let step = 1.0; // Default step for simple ranges
        let mut current = start;
        while current >= end {
            values.push(validate_crf(current)?);
            current -= step;
            // Handle floating point precision issues
            current = (current * 1000.0).round() / 1000.0;
        }
        return Ok(values);
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
pub fn validate_descending(values: &[f64]) -> Result<()> {
    if values.windows(2).any(|pair| pair[0] <= pair[1]) {
        Err(eyre!("Sequence contains non-descending values"))
    } else {
        Ok(())
    }
}
