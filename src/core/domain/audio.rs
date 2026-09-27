use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioFeatures {
    pub energy: u8,
    pub brightness: u8,
}

impl Audio {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
        }
    }

    /// Root-mean-square energy mapped to a compact presentation signal.
    pub fn energy(&self) -> u8 {
        features(&self.samples).energy
    }

    /// A short, playback-synchronised window. Energy follows loudness while
    /// brightness approximates high-frequency content from sample deltas.
    pub fn features_at(&self, elapsed: Duration, window: Duration) -> AudioFeatures {
        if self.samples.is_empty() || self.sample_rate == 0 {
            return AudioFeatures {
                energy: 0,
                brightness: 0,
            };
        }
        let center = (elapsed.as_secs_f64() * f64::from(self.sample_rate)) as usize;
        let width = (window.as_secs_f64() * f64::from(self.sample_rate)) as usize;
        let half = width.max(1) / 2;
        let start = center.saturating_sub(half).min(self.samples.len());
        let end = center.saturating_add(half).min(self.samples.len());
        features(&self.samples[start..end])
    }
}

fn features(samples: &[f32]) -> AudioFeatures {
    if samples.is_empty() {
        return AudioFeatures {
            energy: 0,
            brightness: 0,
        };
    }
    let mean_square = samples
        .iter()
        .map(|sample| sample.clamp(-1.0, 1.0).powi(2))
        .sum::<f32>()
        / samples.len() as f32;
    let rms = mean_square.sqrt().clamp(0.0, 1.0);
    let difference_rms = if samples.len() < 2 {
        0.0
    } else {
        (samples
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).powi(2))
            .sum::<f32>()
            / (samples.len() - 1) as f32)
            .sqrt()
    };
    let brightness = if rms <= f32::EPSILON {
        0.0
    } else {
        (difference_rms / (2.0 * rms)).clamp(0.0, 1.0)
    };
    AudioFeatures {
        energy: (rms * u8::MAX as f32).round() as u8,
        brightness: (brightness * u8::MAX as f32).round() as u8,
    }
}
