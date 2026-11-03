use jagua_rs::Instant;
use std::time::Duration;

/// Generic trait for any struct that can determine if the optimization process should terminate.
pub trait Terminator {
    /// Checks if the termination condition is met.
    ///
    /// A deadline is inclusive: once `now >= deadline` the search must stop.
    fn kill(&self) -> bool;

    /// Sets a new timeout duration
    fn new_timeout(&mut self, timeout: Duration);

    /// Returns the instant when a timeout was set, if any
    fn timeout_at(&self) -> Option<Instant>;

    /// Arms a wall-clock deadline for the whole search (exploration + compression).
    ///
    /// Phase timeouts requested afterwards are clamped so they cannot run past it.
    /// The default does nothing; the CLI terminator uses this to enforce `-t`.
    fn arm_total_deadline(&mut self, total: Duration) {
        let _ = total;
    }
}

pub fn clamp_deadline(now: Instant, timeout: Duration, hard: Option<Instant>) -> Instant {
    let end = now.checked_add(timeout).unwrap_or(now);
    match hard {
        Some(hard) if end > hard => hard,
        _ => end,
    }
}

#[derive(Debug, Clone)]
pub struct BasicTerminator {
    pub timeout: Option<Instant>,
    pub hard_deadline: Option<Instant>,
}

impl Default for BasicTerminator {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicTerminator {
    pub fn new() -> Self {
        Self {
            timeout: None,
            hard_deadline: None,
        }
    }
}

impl Terminator for BasicTerminator {
    fn kill(&self) -> bool {
        let now = Instant::now();
        self.timeout.is_some_and(|timeout| now >= timeout)
            || self.hard_deadline.is_some_and(|deadline| now >= deadline)
    }

    fn new_timeout(&mut self, timeout: Duration) {
        self.timeout = Some(clamp_deadline(Instant::now(), timeout, self.hard_deadline));
    }

    fn timeout_at(&self) -> Option<Instant> {
        self.timeout
    }

    fn arm_total_deadline(&mut self, total: Duration) {
        self.hard_deadline = Some(Instant::now() + total);
    }
}