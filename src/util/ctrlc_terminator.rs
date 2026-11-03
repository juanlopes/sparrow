use crate::util::terminator::{clamp_deadline, Terminator};
use jagua_rs::Instant;
use log::warn;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CtrlCTerminator {
    pub timeout: Option<Instant>,
    /// Wall-clock instant that ends the whole run. Not cleared when a phase starts.
    pub hard_deadline: Option<Instant>,
    pub ctrlc: Arc<AtomicBool>,
}

impl Default for CtrlCTerminator {
    fn default() -> Self {
        Self::new()
    }
}

impl CtrlCTerminator {
    /// Sets up the handler for Ctrl-C (only call once)
    pub fn new() -> Self {
        let ctrlc = Arc::new(AtomicBool::new(false));
        let c = ctrlc.clone();

        ctrlc::set_handler(move || {
            warn!(" terminating...");
            c.store(true, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");

        Self {
            timeout: None,
            hard_deadline: None,
            ctrlc,
        }
    }
}

impl Terminator for CtrlCTerminator {
    fn kill(&self) -> bool {
        let now = Instant::now();
        self.timeout.is_some_and(|timeout| now >= timeout)
            || self.hard_deadline.is_some_and(|deadline| now >= deadline)
            || self.ctrlc.load(Ordering::SeqCst)
    }

    fn new_timeout(&mut self, timeout: Duration) {
        // Reset the Ctrl-C flag and set a new timeout, never past the hard deadline.
        self.ctrlc.store(false, Ordering::SeqCst);
        self.timeout = Some(clamp_deadline(Instant::now(), timeout, self.hard_deadline));
    }

    fn timeout_at(&self) -> Option<Instant> {
        self.timeout
    }

    fn arm_total_deadline(&mut self, total: Duration) {
        self.hard_deadline = Some(Instant::now() + total);
    }
}