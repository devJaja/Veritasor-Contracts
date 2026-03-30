//! Rate-limit and burst-control tests for the attestation contract.

extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, BytesN, Env, String};

fn setup() -> (Env, AttestationContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &0u64);
    (env, client, admin)
}

fn set_ledger_timestamp(env: &Env, ts: u64) {
    env.ledger().set(LedgerInfo {
        timestamp: ts,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 3_110_400,
    });
}

fn submit(client: &AttestationContractClient<'_>, env: &Env, business: &Address, index: u32) {
    let period = String::from_str(env, &std::format!("2026-{:02}", index));
    let root = BytesN::from_array(env, &[index as u8; 32]);
    let nonce = client.get_replay_nonce(business, &crate::NONCE_CHANNEL_BUSINESS);
    client.submit_attestation(
        business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &nonce,
    );
}

fn configure_rate_limit(
    client: &AttestationContractClient<'_>,
    max_submissions: u32,
    window_seconds: u64,
    burst_max_submissions: u32,
    burst_window_seconds: u64,
    enabled: bool,
    nonce: u64,
) {
    client.configure_rate_limit(
        &max_submissions,
        &window_seconds,
        &burst_max_submissions,
        &burst_window_seconds,
        &enabled,
        &nonce,
    );
}

#[test]
fn test_configure_rate_limit_with_burst_controls() {
    let (_env, client, admin) = setup();

    assert!(client.get_rate_limit_config().is_none());
    assert_eq!(client.get_replay_nonce(&admin, &crate::NONCE_CHANNEL_ADMIN), 1);

    configure_rate_limit(&client, 5, 3600, 2, 60, true, 1);

    let config = client.get_rate_limit_config().unwrap();
    assert_eq!(config.max_submissions, 5);
    assert_eq!(config.window_seconds, 3600);
    assert_eq!(config.burst_max_submissions, 2);
    assert_eq!(config.burst_window_seconds, 60);
    assert!(config.enabled);
    assert_eq!(client.get_replay_nonce(&admin, &crate::NONCE_CHANNEL_ADMIN), 2);
}

#[test]
#[should_panic(expected = "burst_max_submissions must be greater than zero")]
fn test_configure_zero_burst_max_rejected() {
    let (_env, client, _admin) = setup();
    configure_rate_limit(&client, 5, 3600, 0, 60, true, 1);
}

#[test]
#[should_panic(expected = "burst_window_seconds must be greater than zero")]
fn test_configure_zero_burst_window_rejected() {
    let (_env, client, _admin) = setup();
    configure_rate_limit(&client, 5, 3600, 2, 0, true, 1);
}

#[test]
#[should_panic(expected = "burst_max_submissions must be less than or equal to max_submissions")]
fn test_configure_burst_above_window_limit_rejected() {
    let (_env, client, _admin) = setup();
    configure_rate_limit(&client, 2, 3600, 3, 60, true, 1);
}

#[test]
#[should_panic(expected = "burst_window_seconds must be less than or equal to window_seconds")]
fn test_configure_burst_window_above_main_window_rejected() {
    let (_env, client, _admin) = setup();
    configure_rate_limit(&client, 5, 60, 2, 120, true, 1);
}

#[test]
#[should_panic(expected = "nonce mismatch")]
fn test_configure_rate_limit_replay_nonce_rejected() {
    let (_env, client, _admin) = setup();
    configure_rate_limit(&client, 5, 3600, 2, 60, true, 1);
    configure_rate_limit(&client, 5, 3600, 2, 60, true, 1);
}

#[test]
fn test_submit_within_full_and_burst_limits() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);

    configure_rate_limit(&client, 4, 3600, 2, 60, true, 1);

    set_ledger_timestamp(&env, 1_000);
    submit(&client, &env, &business, 1);
    set_ledger_timestamp(&env, 1_001);
    submit(&client, &env, &business, 2);

    assert_eq!(client.get_submission_window_count(&business), 2);
    assert_eq!(client.get_submission_burst_count(&business), 2);
    assert_eq!(client.get_replay_nonce(&business, &crate::NONCE_CHANNEL_BUSINESS), 2);
}

#[test]
#[should_panic(expected = "burst rate limit exceeded")]
fn test_burst_limit_exceeded_before_full_window_limit() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);

    configure_rate_limit(&client, 5, 3600, 2, 60, true, 1);

    set_ledger_timestamp(&env, 1_000);
    submit(&client, &env, &business, 1);
    set_ledger_timestamp(&env, 1_001);
    submit(&client, &env, &business, 2);
    set_ledger_timestamp(&env, 1_002);
    submit(&client, &env, &business, 3);
}

#[test]
#[should_panic(expected = "rate limit exceeded")]
fn test_full_window_limit_exceeded_after_burst_window_resets() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);

    configure_rate_limit(&client, 3, 3600, 2, 60, true, 1);

    set_ledger_timestamp(&env, 1_000);
    submit(&client, &env, &business, 1);
    set_ledger_timestamp(&env, 1_001);
    submit(&client, &env, &business, 2);
    set_ledger_timestamp(&env, 1_100);
    submit(&client, &env, &business, 3);

    assert_eq!(client.get_submission_window_count(&business), 3);
    assert_eq!(client.get_submission_burst_count(&business), 1);

    set_ledger_timestamp(&env, 1_200);
    submit(&client, &env, &business, 4);
}

#[test]
fn test_burst_window_expiry_restores_short_term_capacity() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);

    configure_rate_limit(&client, 5, 3600, 2, 60, true, 1);

    set_ledger_timestamp(&env, 1_000);
    submit(&client, &env, &business, 1);
    set_ledger_timestamp(&env, 1_001);
    submit(&client, &env, &business, 2);

    set_ledger_timestamp(&env, 1_062);
    submit(&client, &env, &business, 3);

    assert_eq!(client.get_submission_window_count(&business), 3);
    assert_eq!(client.get_submission_burst_count(&business), 1);
}

#[test]
fn test_exact_cutoff_expires_from_burst_window() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);

    configure_rate_limit(&client, 5, 100, 2, 10, true, 1);

    set_ledger_timestamp(&env, 1_000);
    submit(&client, &env, &business, 1);
    set_ledger_timestamp(&env, 1_001);
    submit(&client, &env, &business, 2);

    set_ledger_timestamp(&env, 1_010);
    assert_eq!(client.get_submission_window_count(&business), 2);
    assert_eq!(client.get_submission_burst_count(&business), 1);
}

#[test]
fn test_multiple_businesses_have_independent_burst_counters() {
    let (env, client, _admin) = setup();
    let business_a = Address::generate(&env);
    let business_b = Address::generate(&env);

    configure_rate_limit(&client, 4, 3600, 2, 60, true, 1);

    set_ledger_timestamp(&env, 1_000);
    submit(&client, &env, &business_a, 1);
    submit(&client, &env, &business_b, 2);
    set_ledger_timestamp(&env, 1_001);
    submit(&client, &env, &business_a, 3);

    assert_eq!(client.get_submission_window_count(&business_a), 2);
    assert_eq!(client.get_submission_burst_count(&business_a), 2);
    assert_eq!(client.get_submission_window_count(&business_b), 1);
    assert_eq!(client.get_submission_burst_count(&business_b), 1);
}

#[test]
fn test_no_config_means_no_limit_and_zero_counts() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);

    set_ledger_timestamp(&env, 1_000);
    for i in 1..=4 {
        submit(&client, &env, &business, i);
    }

    assert_eq!(client.get_submission_window_count(&business), 0);
    assert_eq!(client.get_submission_burst_count(&business), 0);
    assert_eq!(client.get_business_count(&business), 4);
}

#[test]
fn test_disabled_rate_limit_preserves_unlimited_submissions() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);

    configure_rate_limit(&client, 1, 3600, 1, 60, false, 1);

    set_ledger_timestamp(&env, 1_000);
    submit(&client, &env, &business, 1);
    submit(&client, &env, &business, 2);
    submit(&client, &env, &business, 3);

    assert_eq!(client.get_submission_window_count(&business), 0);
    assert_eq!(client.get_submission_burst_count(&business), 0);
    assert_eq!(client.get_business_count(&business), 3);
}
