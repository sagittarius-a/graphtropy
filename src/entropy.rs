use rayon::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Algorithm {
    Shannon,
    ChiSquared,
    ByteFrequency,
}

impl Algorithm {
    pub fn label(&self) -> &'static str {
        match self {
            Algorithm::Shannon => "Shannon Entropy",
            Algorithm::ChiSquared => "Chi-Squared",
            Algorithm::ByteFrequency => "Byte Frequency",
        }
    }

    pub fn y_range(&self) -> (f64, f64) {
        match self {
            Algorithm::Shannon => (0.0, 8.0),
            Algorithm::ChiSquared => (0.0, 1.0),
            Algorithm::ByteFrequency => (0.0, 1.0),
        }
    }

    pub fn y_label(&self) -> &'static str {
        match self {
            Algorithm::Shannon => "Entropy (bits/byte)",
            Algorithm::ChiSquared => "Chi-Squared (normalized)",
            Algorithm::ByteFrequency => "Unique Bytes (ratio)",
        }
    }
}

pub const ALL_ALGORITHMS: [Algorithm; 3] = [
    Algorithm::Shannon,
    Algorithm::ChiSquared,
    Algorithm::ByteFrequency,
];

pub struct EntropyData {
    pub points: Vec<(f64, f64)>,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
}

pub fn compute(data: &[u8], block_size: usize, step: usize, algorithm: Algorithm) -> EntropyData {
    let offsets: Vec<usize> = (0..data.len().saturating_sub(block_size.saturating_sub(1)))
        .step_by(step.max(1))
        .collect();

    let compute_fn = match algorithm {
        Algorithm::Shannon => shannon_entropy,
        Algorithm::ChiSquared => chi_squared,
        Algorithm::ByteFrequency => byte_frequency,
    };

    let points: Vec<(f64, f64)> = offsets
        .par_iter()
        .map(|&offset| {
            let end = (offset + block_size).min(data.len());
            let block = &data[offset..end];
            (offset as f64, compute_fn(block))
        })
        .collect();

    if points.is_empty() {
        return EntropyData {
            points,
            min: 0.0,
            max: 0.0,
            avg: 0.0,
        };
    }

    let mut min = f64::MAX;
    let mut max = f64::MIN;
    let mut sum = 0.0;
    for &(_, v) in &points {
        min = min.min(v);
        max = max.max(v);
        sum += v;
    }
    let avg = sum / points.len() as f64;

    EntropyData {
        points,
        min,
        max,
        avg,
    }
}

fn shannon_entropy(block: &[u8]) -> f64 {
    let mut counts = [0u32; 256];
    for &b in block {
        counts[b as usize] += 1;
    }
    let len = block.len() as f64;
    let mut entropy = 0.0f64;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / len;
            entropy -= p * p.log2();
        }
    }
    entropy
}

fn chi_squared(block: &[u8]) -> f64 {
    let mut counts = [0u32; 256];
    for &b in block {
        counts[b as usize] += 1;
    }
    let expected = block.len() as f64 / 256.0;
    let mut chi2 = 0.0f64;
    for &c in &counts {
        let diff = c as f64 - expected;
        chi2 += diff * diff / expected;
    }
    let max_chi2 = block.len() as f64 * 255.0;
    1.0 - (chi2 / max_chi2).min(1.0)
}

fn byte_frequency(block: &[u8]) -> f64 {
    let mut seen = [false; 256];
    for &b in block {
        seen[b as usize] = true;
    }
    let unique = seen.iter().filter(|&&s| s).count();
    unique as f64 / 256.0
}
