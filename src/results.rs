use std::time::{Duration, Instant};

/// Stores stats from a typing test.
#[derive(Clone)]
pub struct RustypexResults {
    pub total_words: usize,
    pub total_chars_typed: usize,
    pub total_chars_in_text: usize,
    pub total_char_errors: usize,
    pub final_chars_typed_correctly: usize,
    pub final_uncorrected_errors: usize,
    pub started_at: Instant,
    pub ended_at: Instant,
}

impl RustypexResults {
    pub fn duration(&self) -> Duration {
        self.ended_at.duration_since(self.started_at)
    }

    pub fn accuracy(&self) -> f64 {
        (self.total_chars_typed as isize - self.total_char_errors as isize) as f64
            / self.total_chars_typed as f64
    }

    pub fn wpm(&self) -> f64 {
        (self.final_chars_typed_correctly as f64 / 5.0 - self.final_uncorrected_errors as f64)
            .max(0.0) as f64
            / (self.duration().as_secs_f64() / 60.0)
    }
}
