use std::collections::HashMap;

use eyre::{Ok, OptionExt, Result};

#[derive(Debug, Clone, Copy, Default)]
pub struct Score {
    pub frame: u32,
    pub value: f64,
}

#[derive(Debug)]
pub struct ScoreList {
    pub scores: Vec<Score>,
}

#[derive(Debug)]
pub struct Percentile {
    pub n: u32,
    pub score: Score,
}

#[derive(Debug)]
pub struct PercentileList {
    pub percentiles: Vec<Percentile>,
}

#[derive(Debug)]
pub struct Mode {
    pub value: u32,
    pub count: usize,
}

pub fn mean(score_list: &ScoreList) -> f64 {
    score_list
        .scores
        .iter()
        .map(|score| score.value)
        .sum::<f64>()
        / score_list.scores.len() as f64
}

pub fn variance(score_list: &ScoreList) -> f64 {
    let mean_value = mean(score_list);
    score_list
        .scores
        .iter()
        .map(|score| (score.value - mean_value).powi(2))
        .sum::<f64>()
        / score_list.scores.len() as f64
}

pub fn standard_deviation(score_list: &ScoreList) -> f64 {
    variance(score_list).sqrt()
}

pub fn max(score_list: &ScoreList) -> Result<ScoreList> {
    let max_score = score_list
        .scores
        .iter()
        .map(|score| score.value)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .ok_or_eyre("Error getting max score")?;

    let scores = score_list
        .scores
        .iter()
        .filter(|score| score.value == max_score)
        .map(|score| Score {
            frame: score.frame,
            value: score.value,
        })
        .collect::<Vec<_>>();

    Ok(ScoreList { scores })
}

pub fn min(score_list: &ScoreList) -> Result<ScoreList> {
    let min_score = score_list
        .scores
        .iter()
        .map(|score| score.value)
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .ok_or_eyre("Error getting max score")?;

    let scores = score_list
        .scores
        .iter()
        .filter(|score| score.value == min_score)
        .map(|score| Score {
            frame: score.frame,
            value: score.value,
        })
        .collect::<Vec<_>>();

    Ok(ScoreList { scores })
}

pub fn percentiles(score_list: &ScoreList) -> Result<PercentileList> {
    if score_list.scores.is_empty() {
        return Err(eyre::eyre!("Data is empty"));
    }

    // Sort data by score
    let mut sorted = score_list.scores.to_vec();
    sorted.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap());

    // Percentile ranks to compute
    let i_percentiles = [0, 5, 10, 20, 25, 50, 75, 80, 90, 95, 100];

    let n = sorted.len();
    let mut percentiles = Vec::new();

    for &p in &i_percentiles {
        // Compute the rank index (rounded to nearest rank, clamped to end)
        let rank = ((p as f64 / 100.0) * (n as f64 - 1.0)).round() as usize;
        let clamped_rank = rank.min(n - 1);
        let value = sorted[clamped_rank];
        percentiles.push(Percentile { n: p, score: value });
    }

    Ok(PercentileList { percentiles })
}

pub fn median(score_list: &ScoreList) -> Result<ScoreList> {
    if score_list.scores.is_empty() {
        return Err(eyre::eyre!("Data is empty"));
    }

    let mut sorted = score_list.scores.to_vec();
    sorted.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap());

    let mid = sorted.len() / 2;

    let median_values = if sorted.len() % 2 == 1 {
        // If the number of items is odd, return the single middle value
        vec![sorted[mid]]
    } else {
        // If even, return both middle values
        vec![sorted[mid - 1], sorted[mid]]
    };

    Ok(ScoreList {
        scores: median_values,
    })
}

// pub fn mode(data: &[(u32, f64)]) -> Result<Vec<(u32, f64)>> {
//     // Step 1: Build a frequency map of i32-converted scores
//     let mut freq_map: HashMap<i32, usize> = HashMap::new();

//     for &(_, score) in data {
//         let key = score.round() as i32;
//         *freq_map.entry(key).or_insert(0) += 1;
//     }

//     // Step 2: Find the i32 mode (most frequent value)
//     let Some((&mode_key, _)) = freq_map.iter().max_by_key(|&(_, count)| count) else {
//         return Err(eyre::eyre!("No mode found"));
//     };

//     // Step 3: Collect all original (index, score) where converted score == mode
//     let result = data
//         .iter()
//         .cloned()
//         .filter(|&(_, score)| (score.round() as i32) == mode_key)
//         .collect::<Vec<_>>();

//     Ok(result)
// }

pub fn mode(score_list: &ScoreList) -> Result<Mode> {
    let mut freq_map: HashMap<u32, usize> = HashMap::new();

    for score in &score_list.scores {
        let key = score.value.round() as u32;
        *freq_map.entry(key).or_insert(0) += 1;
    }

    let Some((&mode_key, count)) = freq_map.iter().max_by_key(|&(_, count)| count) else {
        return Err(eyre::eyre!("No mode found"));
    };

    Ok(Mode {
        value: mode_key,
        count: *count,
    })
}

use std::fmt::Write; // for write! macro

pub fn get_stats(score_list: &ScoreList) -> Result<String> {
    let mean = mean(score_list);
    let deviation = standard_deviation(score_list);
    let median = median(score_list)?;
    let mode = mode(score_list)?;
    let percentiles = percentiles(score_list)?;
    let max = max(score_list)?;
    let min = min(score_list)?;

    let mut output = String::new();

    writeln!(output, "STATS - SSIMU2")?;
    writeln!(output, "################################")?;
    writeln!(output, "Mean: {:.4}", mean)?;
    writeln!(output, "Standard Deviation: {:.4}", deviation)?;
    writeln!(output, "Mode: {:.4}, count: {:.4}", mode.value, mode.count)?;

    write!(output, "Median: ")?;
    for score in &median.scores {
        write!(
            output,
            "Frame: {:.4} - Score {:.4}, ",
            score.frame, score.value
        )?;
    }
    writeln!(output)?;

    write!(output, "Min: ")?;
    for score in &min.scores {
        write!(
            output,
            "Frame: {:.4} - Score: {:.4}, ",
            score.frame, score.value
        )?;
    }
    writeln!(output)?;

    write!(output, "Max: ")?;
    for score in &max.scores {
        write!(
            output,
            "Frame: {:.4} - Score: {:.4}, ",
            score.frame, score.value
        )?;
    }
    writeln!(output)?;

    writeln!(output, "Percentiles:")?;
    for percentile in &percentiles.percentiles {
        writeln!(
            output,
            "{:03} percentile: Frame:{:06}, Score:{:.4}",
            percentile.n, percentile.score.frame, percentile.score.value
        )?;
    }

    Ok(output)
}
