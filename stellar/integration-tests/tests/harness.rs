//! Chaos harness for Stellar integration tests.
//!
//! Wraps Soroban contract client invocations with an injectable failure policy.
//! Controlled by environment variables:
//!
//! | Variable | Default | Description |
//! |---|---|---|
//! | `WRAITHCHAOS_MODE` | `0` | Set to `1` to enable chaos injection |
//! | `WRAITHCHAOS_FAILURE_RATE` | `0.3` | Probability `[0.0, 1.0]` that any single op fails |
//! | `WRAITHCHAOS_SEED` | `0` | RNG seed for deterministic failure sequences |
//!
//! # Failure Modes
//!
//! Each mode simulates a real-world RPC failure observed against Horizon /
//! Soroban RPC endpoints.
//!
//! | Mode | Simulates | Retry policy | Bail after |
//! |---|---|---|---|
//! | `Http500` | Server returns HTTP 500 | Retry up to 3 times | 3 failures |
//! | `Timeout` | RPC does not respond within deadline | Retry once (2x backoff) | 2 failures |
//! | `WrongLedger` | Response references a stale ledger sequence | No retry | 1 failure |
//! | `EmptyResponse` | RPC returns valid HTTP but empty body | Retry once | 2 failures |

use std::cell::RefCell;

// ── Configuration ────────────────────────────────────────────────────────────

pub struct ChaosConfig {
    pub enabled: bool,
    pub failure_rate: f64,
    pub seed: u64,
}

impl ChaosConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("WRAITHCHAOS_MODE")
            .map(|v| v == "1")
            .unwrap_or(false);
        let failure_rate = std::env::var("WRAITHCHAOS_FAILURE_RATE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.3);
        let seed = std::env::var("WRAITHCHAOS_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        Self {
            enabled,
            failure_rate,
            seed,
        }
    }
}

// ── Failure Modes ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureMode {
    Http500,
    Timeout,
    WrongLedger,
    EmptyResponse,
}

impl FailureMode {
    pub const ALL: [FailureMode; 4] = [
        FailureMode::Http500,
        FailureMode::Timeout,
        FailureMode::WrongLedger,
        FailureMode::EmptyResponse,
    ];

    /// Maximum number of retries before bailing.
    ///
    /// | Mode | Max retries |
    /// |---|---|
    /// | `Http500` | 3 |
    /// | `Timeout` | 1 |
    /// | `WrongLedger` | 0 |
    /// | `EmptyResponse` | 1 |
    pub fn max_retries(&self) -> u32 {
        match self {
            FailureMode::Http500 => 3,
            FailureMode::Timeout => 1,
            FailureMode::WrongLedger => 0,
            FailureMode::EmptyResponse => 1,
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.max_retries() > 0
    }
}

// ── Simple PRNG (xorshift64) ────────────────────────────────────────────────

struct ChaoticRng {
    state: u64,
}

impl ChaoticRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_f64(&mut self) -> f64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as f64) / (u64::MAX as f64)
    }

    fn next_mode(&mut self) -> FailureMode {
        let idx = (self.next_f64() * 4.0) as usize % 4;
        FailureMode::ALL[idx]
    }
}

// ── Chaos Client ─────────────────────────────────────────────────────────────

/// Errors produced by chaos injection (distinct from contract errors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChaosError {
    /// The operation failed after exhausting all retries.
    MaxRetriesExceeded(FailureMode),
    /// A wrong-ledger response was received (non-retryable).
    WrongLedger,
    /// An empty response body was received after all retries.
    EmptyResponse,
    /// Chaos is disabled; this error should never appear in normal usage.
    ChaosDisabled,
}

/// A wrapper that injects failures into contract client operations.
///
/// Each operation is wrapped so that, when chaos mode is enabled, a
/// configurable fraction of calls will fail according to the active
/// `FailureMode`.
///
/// # Retry / Bail Policy (applies to every wrapped op)
///
/// | Failure mode | Retries | Backoff | Bail error |
/// |---|---|---|---|
/// | `Http500` | up to 3 | 100ms x attempt | `ChaosError::MaxRetriesExceeded` |
/// | `Timeout` | 1 | 200ms x attempt | `ChaosError::MaxRetriesExceeded` |
/// | `WrongLedger` | 0 | n/a | `ChaosError::WrongLedger` |
/// | `EmptyResponse` | 1 | 100ms x attempt | `ChaosError::EmptyResponse` |
pub struct ChaosClient {
    config: ChaosConfig,
    rng: RefCell<ChaoticRng>,
}

impl ChaosClient {
    pub fn from_env() -> Self {
        let config = ChaosConfig::from_env();
        let rng = RefCell::new(ChaoticRng::new(config.seed));
        Self { config, rng }
    }

    pub fn new(enabled: bool, failure_rate: f64, seed: u64) -> Self {
        let config = ChaosConfig {
            enabled,
            failure_rate,
            seed,
        };
        let rng = RefCell::new(ChaoticRng::new(seed));
        Self { config, rng }
    }

    pub fn is_chaos_enabled(&self) -> bool {
        self.config.enabled
    }

    fn should_fail(&self) -> Option<FailureMode> {
        if !self.config.enabled {
            return None;
        }
        let roll = self.rng.borrow_mut().next_f64();
        if roll < self.config.failure_rate {
            Some(self.rng.borrow_mut().next_mode())
        } else {
            None
        }
    }

    /// Execute an operation with chaos injection.
    ///
    /// `op_name` is a human-readable label for logging.
    /// `op` is the closure that performs the real work.
    ///
    /// Retry / bail policy is derived from the selected `FailureMode`
    /// (see table at struct-level docs).
    pub fn execute<F, T, E>(&self, op_name: &str, op: F) -> Result<T, ChaosError>
    where
        F: Fn() -> Result<T, E>,
    {
        let mode = match self.should_fail() {
            Some(m) => m,
            None => return op().map_err(|_| ChaosError::ChaosDisabled),
        };

        let max = mode.max_retries();

        for attempt in 0..=max {
            if attempt > 0 {
                Self::backoff(attempt, &mode);
            }

            eprintln!(
                "[chaos] {}: injecting {:?} (attempt {}/{})",
                op_name,
                mode,
                attempt + 1,
                max + 1
            );

            match mode {
                FailureMode::WrongLedger => {
                    return Err(ChaosError::WrongLedger);
                }
                FailureMode::EmptyResponse => {
                    if attempt >= max {
                        return Err(ChaosError::EmptyResponse);
                    }
                }
                FailureMode::Http500 | FailureMode::Timeout => {
                    if attempt >= max {
                        return Err(ChaosError::MaxRetriesExceeded(mode));
                    }
                }
            }
        }

        Err(ChaosError::MaxRetriesExceeded(mode))
    }

    fn backoff(attempt: u32, mode: &FailureMode) {
        let base_ms: u64 = match mode {
            FailureMode::Http500 => 100,
            FailureMode::Timeout => 200,
            FailureMode::EmptyResponse => 100,
            FailureMode::WrongLedger => 0,
        };
        let ms = base_ms * attempt as u64;
        if ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaos_disabled_always_succeeds() {
        let client = ChaosClient::new(false, 1.0, 42);
        let result = client.execute("test", || Ok::<_, ()>(42));
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn chaos_enabled_high_rate_always_fails() {
        let client = ChaosClient::new(true, 1.0, 42);
        let result = client.execute("test", || Ok::<_, ()>(42));
        assert!(result.is_err());
    }

    #[test]
    fn chaos_zero_rate_always_succeeds() {
        let client = ChaosClient::new(true, 0.0, 42);
        let result = client.execute("test", || Ok::<_, ()>(42));
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn failure_mode_retry_counts() {
        assert_eq!(FailureMode::Http500.max_retries(), 3);
        assert_eq!(FailureMode::Timeout.max_retries(), 1);
        assert_eq!(FailureMode::WrongLedger.max_retries(), 0);
        assert_eq!(FailureMode::EmptyResponse.max_retries(), 1);
    }

    #[test]
    fn deterministic_with_same_seed() {
        let c1 = ChaosClient::new(true, 0.5, 99);
        let c2 = ChaosClient::new(true, 0.5, 99);
        let mut r1 = std::vec::Vec::new();
        let mut r2 = std::vec::Vec::new();
        for _ in 0..100 {
            r1.push(c1.execute("t", || Ok::<_, ()>(1)).is_ok());
            r2.push(c2.execute("t", || Ok::<_, ()>(1)).is_ok());
        }
        assert_eq!(r1, r2);
    }
}
