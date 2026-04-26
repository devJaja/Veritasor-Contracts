//! Tests for time-locked revenue stream contract.

use super::{VestingSchedule, *};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env, String};

#[cfg(test)]
use veritasor_attestation::{AttestationContract, AttestationContractClient};

fn setup(
    env: &Env,
) -> (
    Address,
    Address,
    RevenueStreamContractClient<'static>,
    Address,
    AttestationContractClient<'static>,
    Address,
    Address,
) {
    let admin = Address::generate(env);
    let stream_contract_id = env.register(RevenueStreamContract, ());
    let stream_client = RevenueStreamContractClient::new(env, &stream_contract_id);
    stream_client.initialize(&admin, &0u64);
    let attestation_id = env.register(AttestationContract, ());
    let attestation_client = AttestationContractClient::new(env, &attestation_id);
    attestation_client.initialize(&admin, &0u64);
    let token_admin = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin);
    let token = token_contract.address().clone();
    let beneficiary = Address::generate(env);
    (
        admin,
        stream_contract_id,
        stream_client,
        attestation_id,
        attestation_client,
        token,
        beneficiary,
    )
}

fn lump_none() -> VestingSchedule {
    VestingSchedule::Lump { cliff: None }
}

#[test]
fn test_create_and_release_stream() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &lump_none(),
    );
    assert_eq!(stream_id, 0);
    let stream = stream_client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.released_amount, 0);
    stream_client.release(&stream_id);
    let stream = stream_client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.released_amount, amount);
}

#[test]
#[should_panic(expected = "attestation not found")]
fn test_release_without_attestation_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, _attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &lump_none(),
    );
    stream_client.release(&stream_id);
}

#[test]
#[should_panic(expected = "attestation is revoked")]
fn test_release_when_revoked_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let reason = String::from_str(&env, "test revoke");
    attestation_client.revoke_attestation(&admin, &business, &period, &reason, &1u64);
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &lump_none(),
    );
    stream_client.release(&stream_id);
}

#[test]
#[should_panic(expected = "stream already released")]
fn test_double_release_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &lump_none(),
    );
    stream_client.release(&stream_id);
    stream_client.release(&stream_id);
}

#[test]
fn test_get_stream() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, _attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let amount = 500i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &lump_none(),
    );
    let stream = stream_client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.beneficiary, beneficiary);
    assert_eq!(stream.amount, amount);
    assert_eq!(stream.period, period);
}

#[test]
fn test_multiple_streams() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let amount = 2000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    let id0 = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &String::from_str(&env, "2026-01"),
        &beneficiary,
        &token,
        &1000i128,
        &lump_none(),
    );
    let id1 = stream_client.create_stream(
        &admin,
        &2u64,
        &attestation_id,
        &business,
        &String::from_str(&env, "2026-02"),
        &beneficiary,
        &token,
        &1000i128,
        &lump_none(),
    );
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    attestation_client.submit_attestation(
        &business,
        &String::from_str(&env, "2026-01"),
        &soroban_sdk::BytesN::from_array(&env, &[1u8; 32]),
        &1u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    stream_client.release(&id0);
    assert_eq!(stream_client.get_stream(&id0).unwrap().released_amount, 1000i128);
    assert_eq!(stream_client.get_stream(&id1).unwrap().released_amount, 0i128);
}

#[test]
fn test_release_with_cliff_in_past() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    // Cliff in the past
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &VestingSchedule::Lump {
            cliff: Some(1_000_000_000u64),
        },
    );
    stream_client.release(&stream_id);
    let stream = stream_client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.released_amount, amount);
}

#[test]
#[should_panic(expected = "cliff not reached")]
fn test_release_with_cliff_in_future_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    // Cliff in the future
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &VestingSchedule::Lump {
            cliff: Some(2_000_000_000u64),
        },
    );
    stream_client.release(&stream_id);
}

#[test]
fn test_release_with_cliff_at_current_time() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    // Cliff at current ledger timestamp
    let current_time = env.ledger().timestamp();
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &VestingSchedule::Lump {
            cliff: Some(current_time),
        },
    );
    stream_client.release(&stream_id);
    let stream = stream_client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.released_amount, amount);
}

#[test]
fn test_release_with_no_cliff() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &lump_none(),
    );
    stream_client.release(&stream_id);
    let stream = stream_client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.released_amount, amount);
}

#[test]
fn test_lump_cliff_zero_releases_when_effective_ledger_allows() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    env.ledger().set_timestamp(1u64);
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    // Cliff 0: effective time must be >= 0 (always once ledger >= 0).
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &VestingSchedule::Lump { cliff: Some(0u64) },
    );
    stream_client.release(&stream_id);
    assert_eq!(stream_client.get_stream(&stream_id).unwrap().released_amount, amount);
}

/// Linear: two partial releases, attestation gating for both.
#[test]
fn test_linear_accrual_partial_releases() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000u64);
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    let amount = 1000i128;
    StellarAssetClient::new(&env, &token).mint(&admin, &amount);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &amount,
        &VestingSchedule::Linear {
            accrual_start: 1_000u64,
            accrual_end: 2_000u64,
        },
    );
    env.ledger().set_timestamp(1_500u64);
    assert_eq!(stream_client.get_vested_by_schedule(&stream_id), 500i128);
    stream_client.release(&stream_id);
    assert_eq!(stream_client.get_stream(&stream_id).unwrap().released_amount, 500i128);
    env.ledger().set_timestamp(2_000u64);
    assert_eq!(stream_client.get_vested_by_schedule(&stream_id), 1_000i128);
    stream_client.release(&stream_id);
    assert_eq!(stream_client.get_stream(&stream_id).unwrap().released_amount, 1_000i128);
}

#[test]
#[should_panic(expected = "accrual_start must be before accrual_end")]
fn test_create_rejects_inverted_accrual_range() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, _a, token, beneficiary) = setup(&env);
    StellarAssetClient::new(&env, &token).mint(&admin, &1000i128);
    let business = Address::generate(&env);
    stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &String::from_str(&env, "p1"),
        &beneficiary,
        &token,
        &1000i128,
        &VestingSchedule::Linear {
            accrual_start: 2_000u64,
            accrual_end: 1_000u64,
        },
    );
}

#[test]
#[should_panic(expected = "nothing to claim")]
fn test_release_linear_before_start_fails() {
    let env = Env::default();
    env.ledger().set_timestamp(500u64);
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    attestation_client.submit_attestation(
        &business,
        &period,
        &soroban_sdk::BytesN::from_array(&env, &[1u8; 32]),
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    StellarAssetClient::new(&env, &token).mint(&admin, &1000i128);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &1000i128,
        &VestingSchedule::Linear {
            accrual_start: 1_000u64,
            accrual_end: 2_000u64,
        },
    );
    stream_client.release(&stream_id);
}

#[test]
fn test_pause_freezes_vesting_and_release_blocks() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000u64);
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    attestation_client.submit_attestation(
        &business,
        &period,
        &soroban_sdk::BytesN::from_array(&env, &[1u8; 32]),
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    StellarAssetClient::new(&env, &token).mint(&admin, &1000i128);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &1000i128,
        &VestingSchedule::Linear {
            accrual_start: 1_000u64,
            accrual_end: 3_000u64,
        },
    );
    env.ledger().set_timestamp(2_000u64);
    assert_eq!(stream_client.get_vested_by_schedule(&stream_id), 500i128);
    stream_client.pause(&admin, &2u64);
    assert!(stream_client.is_paused());
    assert_eq!(stream_client.get_vested_by_schedule(&stream_id), 500i128);
    env.ledger().set_timestamp(2_500u64);
    assert_eq!(stream_client.get_vested_by_schedule(&stream_id), 500i128);
}

#[test]
#[should_panic(expected = "contract is paused")]
fn test_release_fails_while_paused() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000u64);
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    attestation_client.submit_attestation(
        &business,
        &period,
        &soroban_sdk::BytesN::from_array(&env, &[1u8; 32]),
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    StellarAssetClient::new(&env, &token).mint(&admin, &1000i128);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &1000i128,
        &lump_none(),
    );
    stream_client.pause(&admin, &2u64);
    stream_client.release(&stream_id);
}

#[test]
fn test_resume_allows_accrual_to_continue() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000u64);
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, attestation_id, attestation_client, token, beneficiary) =
        setup(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    attestation_client.submit_attestation(
        &business,
        &period,
        &soroban_sdk::BytesN::from_array(&env, &[1u8; 32]),
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    StellarAssetClient::new(&env, &token).mint(&admin, &1000i128);
    let stream_id = stream_client.create_stream(
        &admin,
        &1u64,
        &attestation_id,
        &business,
        &period,
        &beneficiary,
        &token,
        &1000i128,
        &VestingSchedule::Linear {
            accrual_start: 1_000u64,
            accrual_end: 2_000u64,
        },
    );
    env.ledger().set_timestamp(1_500u64);
    stream_client.pause(&admin, &2u64);
    // Resume later: "lost" time between pause and accrual_end is made up on the remapped clock.
    env.ledger().set_timestamp(2_000u64);
    stream_client.resume(&admin, &3u64);
    assert!(!stream_client.is_paused());
    // Still half at resume ledger (synthetic time remains 1500).
    assert_eq!(stream_client.get_vested_by_schedule(&stream_id), 500i128);
    env.ledger().set_timestamp(2_500u64);
    assert_eq!(stream_client.get_vested_by_schedule(&stream_id), 1000i128);
    assert_eq!(stream_client.get_effective_vest_ledger(), 2_000u64);
    stream_client.release(&stream_id);
    assert_eq!(stream_client.get_stream(&stream_id).unwrap().released_amount, 1000i128);
}

#[test]
#[should_panic(expected = "already paused")]
fn test_double_pause_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, _a, _ac, _t, _b) = setup(&env);
    stream_client.pause(&admin, &1u64);
    stream_client.pause(&admin, &2u64);
}

#[test]
#[should_panic(expected = "not paused")]
fn test_resume_without_pause_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, _stream_id, stream_client, _a, _ac, _t, _b) = setup(&env);
    stream_client.resume(&admin, &1u64);
}
