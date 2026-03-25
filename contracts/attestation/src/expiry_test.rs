use crate::{AttestationContract, AttestationContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String,
};

fn setup() -> (Env, AttestationContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &0u64);
    (env, client, admin)
}

#[test]
fn submit_without_expiry_succeeds() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);

    client.submit_attestation(&business, &period, &root, &1_000u64, &1u32, &None, &None);

    let stored = client.get_attestation(&business, &period).unwrap();
    assert_eq!(stored.0, root);
    assert_eq!(stored.1, 1_000u64);
    assert_eq!(stored.2, 1u32);
    assert_eq!(stored.5, None);
}

#[test]
fn submit_with_future_expiry_succeeds() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q2");
    let root = BytesN::from_array(&env, &[2u8; 32]);

    env.ledger().set_timestamp(1_000);
    client.submit_attestation(
        &business,
        &period,
        &root,
        &1_050u64,
        &1u32,
        &None,
        &Some(2_000u64),
    );

    let stored = client.get_attestation(&business, &period).unwrap();
    assert_eq!(stored.5, Some(2_000u64));
}

#[test]
#[should_panic(expected = "expiry must be in the future")]
fn submit_with_past_expiry_panics() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q3");
    let root = BytesN::from_array(&env, &[3u8; 32]);

    env.ledger().set_timestamp(2_000);
    client.submit_attestation(
        &business,
        &period,
        &root,
        &1_500u64,
        &1u32,
        &None,
        &Some(1_900u64),
    );
}

#[test]
#[should_panic(expected = "expiry must be in the future")]
fn submit_with_expiry_equal_to_ledger_time_panics() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q4");
    let root = BytesN::from_array(&env, &[4u8; 32]);

    env.ledger().set_timestamp(2_000);
    client.submit_attestation(
        &business,
        &period,
        &root,
        &1_800u64,
        &1u32,
        &None,
        &Some(2_000u64),
    );
}

#[test]
#[should_panic(expected = "expiry must be after attestation timestamp")]
fn submit_with_expiry_before_attestation_timestamp_panics() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2027-Q1");
    let root = BytesN::from_array(&env, &[5u8; 32]);

    env.ledger().set_timestamp(1_000);
    client.submit_attestation(
        &business,
        &period,
        &root,
        &2_000u64,
        &1u32,
        &None,
        &Some(1_999u64),
    );
}

#[test]
fn is_expired_boundary_behavior_is_enforced() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2027-Q2");
    let root = BytesN::from_array(&env, &[6u8; 32]);

    env.ledger().set_timestamp(1_000);
    client.submit_attestation(
        &business,
        &period,
        &root,
        &1_050u64,
        &1u32,
        &None,
        &Some(1_500u64),
    );

    env.ledger().set_timestamp(1_499);
    assert!(!client.is_expired(&business, &period));

    env.ledger().set_timestamp(1_500);
    assert!(client.is_expired(&business, &period));
}

#[test]
fn verify_attestation_fails_after_expiry() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2027-Q3");
    let root = BytesN::from_array(&env, &[7u8; 32]);

    env.ledger().set_timestamp(500);
    client.submit_attestation(
        &business,
        &period,
        &root,
        &550u64,
        &1u32,
        &None,
        &Some(1_000u64),
    );

    env.ledger().set_timestamp(900);
    assert!(client.verify_attestation(&business, &period, &root));

    env.ledger().set_timestamp(1_000);
    assert!(!client.verify_attestation(&business, &period, &root));
}

#[test]
fn expired_attestation_remains_queryable() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2027-Q4");
    let root = BytesN::from_array(&env, &[8u8; 32]);

    env.ledger().set_timestamp(10);
    client.submit_attestation(
        &business,
        &period,
        &root,
        &11u64,
        &1u32,
        &None,
        &Some(20u64),
    );

    env.ledger().set_timestamp(25);
    let stored = client.get_attestation(&business, &period);
    assert!(stored.is_some());
    assert!(client.is_expired(&business, &period));
}

#[test]
fn is_expired_is_false_for_missing_attestation() {
    let (env, client, _admin) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2028-Q1");

    assert!(!client.is_expired(&business, &period));
}
