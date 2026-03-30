//! # Rate Limiting for Attestation Submissions
//!
//! Configurable, per-business rate limiting to prevent abuse and spam
//! submissions. Uses a sliding-window model plus a shorter burst window:
//! each business can submit at most `max_submissions` attestations within
//! any `window_seconds` span, and at most `burst_max_submissions`
//! attestations within any `burst_window_seconds` span.
//!
//! ## Algorithm
//!
//! On every submission the contract:
//!
//! 1. Loads the business's stored timestamps (`Vec<u64>`).
//! 2. Prunes any entries older than `now - window_seconds`.
//! 3. Counts active entries in the full window.
//! 4. Counts active entries in the shorter burst window.
//! 5. If the full-window count is already at the limit, panics with
//!    `"rate limit exceeded"`.
//! 6. If the burst-window count is already at the limit, panics with
//!    `"burst rate limit exceeded"`.
//! 7. After the attestation is successfully stored, records the current
//!    timestamp.
//!
//! ## Backward Compatibility
//!
//! If no `RateLimitConfig` has been stored, or if
//! `RateLimitConfig.enabled == false`, no limits are enforced.

use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::dynamic_fees::DataKey;

/// On-chain rate limit configuration.
///
/// Stored under [`DataKey::RateLimitConfig`]. The admin sets this via
/// `configure_rate_limit`.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimitConfig {
    /// Maximum number of submissions allowed in the full sliding window.
    pub max_submissions: u32,
    /// Full sliding-window duration in seconds.
    pub window_seconds: u64,
    /// Maximum number of submissions allowed in the shorter burst window.
    pub burst_max_submissions: u32,
    /// Shorter burst-window duration in seconds.
    pub burst_window_seconds: u64,
    /// Master switch. When `false`, rate limiting is disabled.
    pub enabled: bool,
}

/// Store the rate limit configuration.
///
/// Validates that both the steady-state and burst controls are well-formed.
pub fn set_rate_limit_config(env: &Env, config: &RateLimitConfig) {
    assert!(
        config.max_submissions > 0,
        "max_submissions must be greater than zero"
    );
    assert!(
        config.window_seconds > 0,
        "window_seconds must be greater than zero"
    );
    assert!(
        config.burst_max_submissions > 0,
        "burst_max_submissions must be greater than zero"
    );
    assert!(
        config.burst_window_seconds > 0,
        "burst_window_seconds must be greater than zero"
    );
    assert!(
        config.burst_max_submissions <= config.max_submissions,
        "burst_max_submissions must be less than or equal to max_submissions"
    );
    assert!(
        config.burst_window_seconds <= config.window_seconds,
        "burst_window_seconds must be less than or equal to window_seconds"
    );

    env.storage()
        .instance()
        .set(&DataKey::RateLimitConfig, config);
}

/// Read the rate limit configuration, if any.
pub fn get_rate_limit_config(env: &Env) -> Option<RateLimitConfig> {
    env.storage().instance().get(&DataKey::RateLimitConfig)
}

/// Read the stored submission timestamps for a business.
fn get_timestamps(env: &Env, business: &Address) -> Vec<u64> {
    env.storage()
        .instance()
        .get(&DataKey::SubmissionTimestamps(business.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Overwrite the stored submission timestamps for a business.
fn set_timestamps(env: &Env, business: &Address, timestamps: &Vec<u64>) {
    env.storage()
        .instance()
        .set(&DataKey::SubmissionTimestamps(business.clone()), timestamps);
}

/// Analyze the current submission history for the configured windows.
///
/// Returns `(active_timestamps, original_len, full_window_count, burst_count)`.
fn analyze_submission_windows(
    env: &Env,
    config: &RateLimitConfig,
    business: &Address,
) -> (Vec<u64>, u32, u32, u32) {
    let now = env.ledger().timestamp();
    let cutoff = now.saturating_sub(config.window_seconds);
    let burst_cutoff = now.saturating_sub(config.burst_window_seconds);

    let stored = get_timestamps(env, business);
    let original_len = stored.len();

    let mut active: Vec<u64> = Vec::new(env);
    let mut window_count: u32 = 0;
    let mut burst_count: u32 = 0;

    for i in 0..stored.len() {
        let ts = stored.get(i).unwrap();
        if ts > cutoff {
            active.push_back(ts);
            window_count += 1;

            if ts > burst_cutoff {
                burst_count += 1;
            }
        }
    }

    (active, original_len, window_count, burst_count)
}

/// Enforce the configured rate limits for `business`.
///
/// Prunes expired entries before checking the full sliding window and the
/// shorter burst window. Writes the pruned timestamps back only when the
/// stored vector actually shrinks.
pub fn check_rate_limit(env: &Env, business: &Address) {
    let config = match get_rate_limit_config(env) {
        Some(c) if c.enabled => c,
        _ => return,
    };

    let (active, original_len, window_count, burst_count) =
        analyze_submission_windows(env, &config, business);

    if active.len() != original_len {
        set_timestamps(env, business, &active);
    }

    assert!(
        window_count < config.max_submissions,
        "rate limit exceeded"
    );
    assert!(
        burst_count < config.burst_max_submissions,
        "burst rate limit exceeded"
    );
}

/// Record the current ledger timestamp for `business`.
///
/// Must be called only after a successful attestation write.
pub fn record_submission(env: &Env, business: &Address) {
    let config = match get_rate_limit_config(env) {
        Some(c) if c.enabled => c,
        _ => return,
    };

    let now = env.ledger().timestamp();
    let (mut active, _, _, _) = analyze_submission_windows(env, &config, business);
    active.push_back(now);

    set_timestamps(env, business, &active);
}

/// Count active submissions for `business` in the full sliding window.
///
/// This is a read-only helper that does not mutate storage.
pub fn get_submission_count(env: &Env, business: &Address) -> u32 {
    let config = match get_rate_limit_config(env) {
        Some(c) if c.enabled => c,
        _ => return 0,
    };

    let (_, _, count, _) = analyze_submission_windows(env, &config, business);
    count
}

/// Count active submissions for `business` in the burst window.
///
/// This is a read-only helper that does not mutate storage.
pub fn get_burst_submission_count(env: &Env, business: &Address) -> u32 {
    let config = match get_rate_limit_config(env) {
        Some(c) if c.enabled => c,
        _ => return 0,
    };

    let (_, _, _, burst_count) = analyze_submission_windows(env, &config, business);
    burst_count
}
