#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{Address, Env, String, Vec};

// ════════════════════════════════════════════════════════════════════
//  Test Helpers
// ════════════════════════════════════════════════════════════════════

fn setup() -> (
    Env,
    RevenueShareContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(RevenueShareContract, ());
    let client = RevenueShareContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attestation_contract = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(token_admin.clone());

    client.initialize(&admin, &0u64, &attestation_contract, &token_id.address());

    (env, client, admin, attestation_contract, token_id.address())
}

fn setup_uninitialized() -> (Env, RevenueShareContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(RevenueShareContract, ());
    let client = RevenueShareContractClient::new(&env, &contract_id);

    (env, client)
}

fn mint(env: &Env, token_addr: &Address, to: &Address, amount: i128) {
    StellarAssetClient::new(env, token_addr).mint(to, &amount);
}

fn admin_nonce(client: &RevenueShareContractClient<'_>, admin: &Address) -> u64 {
    client.get_replay_nonce(admin, &NONCE_CHANNEL_ADMIN)
}

fn configure_stakeholders_as_admin(
    client: &RevenueShareContractClient<'_>,
    admin: &Address,
    stakeholders: &Vec<Stakeholder>,
) {
    let nonce = admin_nonce(client, admin);
    client.configure_stakeholders(&nonce, stakeholders);
}

fn set_attestation_contract_as_admin(
    client: &RevenueShareContractClient<'_>,
    admin: &Address,
    attestation_contract: &Address,
) {
    let nonce = admin_nonce(client, admin);
    client.set_attestation_contract(&nonce, attestation_contract);
}

fn set_token_as_admin(client: &RevenueShareContractClient<'_>, admin: &Address, token: &Address) {
    let nonce = admin_nonce(client, admin);
    client.set_token(&nonce, token);
}

fn stakeholders_with_shares(env: &Env, shares: &[u32]) -> Vec<Stakeholder> {
    let mut stakeholders = Vec::new(env);
    for share in shares {
        stakeholders.push_back(Stakeholder {
            address: Address::generate(env),
            share_bps: *share,
        });
    }
    stakeholders
}

fn equal_stakeholders(env: &Env, count: u32) -> Vec<Stakeholder> {
    let mut stakeholders = Vec::new(env);
    let share_per_stakeholder = 10_000 / count;
    let mut remaining = 10_000;

    for i in 0..count {
        let share = if i == count - 1 {
            remaining
        } else {
            share_per_stakeholder
        };
        stakeholders.push_back(Stakeholder {
            address: Address::generate(env),
            share_bps: share,
        });
        remaining -= share;
    }

    stakeholders
}

fn sum_amounts(amounts: &Vec<i128>) -> i128 {
    let mut total = 0i128;
    for i in 0..amounts.len() {
        total += amounts.get(i).unwrap();
    }
    total
}

fn expected_distribution_amounts(
    env: &Env,
    stakeholders: &Vec<Stakeholder>,
    revenue_amount: i128,
) -> Vec<i128> {
    let mut amounts = Vec::new(env);
    let mut total_distributed = 0i128;

    for i in 0..stakeholders.len() {
        let stakeholder = stakeholders.get(i).unwrap();
        let amount = RevenueShareContract::calculate_share(revenue_amount, stakeholder.share_bps);
        amounts.push_back(amount);
        total_distributed += amount;
    }

    let residual = revenue_amount - total_distributed;
    if residual > 0 {
        let first_amount = amounts.get(0).unwrap();
        amounts.set(0, first_amount + residual);
    }

    amounts
}

fn assert_distribution_invariants(
    env: &Env,
    client: &RevenueShareContractClient<'_>,
    token: &Address,
    business: &Address,
    period: &String,
    stakeholders: &Vec<Stakeholder>,
    revenue_amount: i128,
) {
    let token_client = TokenClient::new(env, token);
    let record = client.get_distribution(business, period).unwrap();
    let expected_amounts = expected_distribution_amounts(env, stakeholders, revenue_amount);

    assert_eq!(record.total_amount, revenue_amount);
    assert_eq!(record.amounts.len(), stakeholders.len());
    assert_eq!(sum_amounts(&record.amounts), revenue_amount);
    assert_eq!(sum_amounts(&expected_amounts), revenue_amount);

    let mut residual = revenue_amount;
    for i in 0..stakeholders.len() {
        let stakeholder = stakeholders.get(i).unwrap();
        let base_amount =
            RevenueShareContract::calculate_share(revenue_amount, stakeholder.share_bps);
        residual -= base_amount;

        let expected = expected_amounts.get(i).unwrap();
        let actual = record.amounts.get(i).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(token_client.balance(&stakeholder.address), expected);

        if i > 0 {
            assert_eq!(actual, base_amount);
        }
    }

    assert!(residual >= 0);
    assert!(residual < stakeholders.len() as i128);
    assert_eq!(
        record.amounts.get(0).unwrap(),
        expected_amounts.get(0).unwrap()
    );
    assert_eq!(token_client.balance(business), 0);
}

// ════════════════════════════════════════════════════════════════════
//  Initialization and Replay Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_initialize_stores_contract_config_and_advances_replay_nonce() {
    let (_env, client, admin, attestation_contract, token) = setup();

    assert_eq!(client.get_admin(), admin.clone());
    assert_eq!(client.get_attestation_contract(), attestation_contract);
    assert_eq!(client.get_token(), token);
    assert_eq!(admin_nonce(&client, &admin), 1);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_twice_panics() {
    let (env, client, _admin, attestation_contract, token) = setup();
    let new_admin = Address::generate(&env);

    client.initialize(&new_admin, &0u64, &attestation_contract, &token);
}

#[test]
fn test_uninitialized_queries_expose_empty_or_error_state() {
    let (env, client) = setup_uninitialized();
    let actor = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");

    assert!(client.try_get_admin().is_err());
    assert!(client.try_get_attestation_contract().is_err());
    assert!(client.try_get_token().is_err());
    assert!(client.get_stakeholders().is_none());
    assert!(client.get_distribution(&actor, &period).is_none());
    assert_eq!(client.get_distribution_count(&actor), 0);
    assert_eq!(client.get_replay_nonce(&actor, &NONCE_CHANNEL_ADMIN), 0);
}

#[test]
fn test_replayed_admin_nonce_is_rejected_without_mutating_state() {
    let (env, client, admin, _attestation, _token) = setup();
    let stakeholders = stakeholders_with_shares(&env, &[6000, 4000]);

    let nonce = admin_nonce(&client, &admin);
    client.configure_stakeholders(&nonce, &stakeholders);
    assert_eq!(admin_nonce(&client, &admin), nonce + 1);

    assert!(client
        .try_configure_stakeholders(&nonce, &stakeholders)
        .is_err());
    assert_eq!(admin_nonce(&client, &admin), nonce + 1);
    assert_eq!(client.get_stakeholders().unwrap(), stakeholders);
}

#[test]
fn test_failed_admin_validation_does_not_consume_nonce() {
    let (env, client, admin, _attestation, _token) = setup();

    let invalid = stakeholders_with_shares(&env, &[5000, 4000]);
    let valid = stakeholders_with_shares(&env, &[6000, 4000]);
    let nonce = admin_nonce(&client, &admin);

    assert!(client.try_configure_stakeholders(&nonce, &invalid).is_err());
    assert_eq!(admin_nonce(&client, &admin), nonce);

    client.configure_stakeholders(&nonce, &valid);
    assert_eq!(admin_nonce(&client, &admin), nonce + 1);
    assert_eq!(client.get_stakeholders().unwrap(), valid);
}

#[test]
fn test_admin_setters_update_state_and_advance_nonce() {
    let (env, client, admin, _attestation, _token) = setup();
    let new_attestation = Address::generate(&env);
    let new_token_admin = Address::generate(&env);
    let new_token = env
        .register_stellar_asset_contract_v2(new_token_admin)
        .address()
        .clone();

    let nonce_before = admin_nonce(&client, &admin);
    set_attestation_contract_as_admin(&client, &admin, &new_attestation);
    assert_eq!(client.get_attestation_contract(), new_attestation);
    assert_eq!(admin_nonce(&client, &admin), nonce_before + 1);

    set_token_as_admin(&client, &admin, &new_token);
    assert_eq!(client.get_token(), new_token);
    assert_eq!(admin_nonce(&client, &admin), nonce_before + 2);
}

// ════════════════════════════════════════════════════════════════════
//  Stakeholder Configuration Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_configure_stakeholders_accepts_valid_configurations() {
    let (env, client, admin, _attestation, _token) = setup();

    let initial = equal_stakeholders(&env, 2);
    configure_stakeholders_as_admin(&client, &admin, &initial);
    assert_eq!(client.get_stakeholders().unwrap(), initial);

    let updated = stakeholders_with_shares(&env, &[5000, 3000, 2000]);
    configure_stakeholders_as_admin(&client, &admin, &updated);
    assert_eq!(client.get_stakeholders().unwrap(), updated);
}

#[test]
#[should_panic(expected = "must have at least one stakeholder")]
fn test_configure_stakeholders_empty_panics() {
    let (env, client, admin, _attestation, _token) = setup();
    let stakeholders = Vec::new(&env);

    client.configure_stakeholders(&admin_nonce(&client, &admin), &stakeholders);
}

#[test]
#[should_panic(expected = "cannot exceed 50 stakeholders")]
fn test_configure_stakeholders_too_many_panics() {
    let (env, client, admin, _attestation, _token) = setup();
    let stakeholders = equal_stakeholders(&env, 51);

    client.configure_stakeholders(&admin_nonce(&client, &admin), &stakeholders);
}

#[test]
#[should_panic(expected = "total shares must equal 10,000 bps (100%)")]
fn test_configure_stakeholders_invalid_total_panics() {
    let (env, client, admin, _attestation, _token) = setup();
    let stakeholders = stakeholders_with_shares(&env, &[5000, 4000]);

    client.configure_stakeholders(&admin_nonce(&client, &admin), &stakeholders);
}

#[test]
#[should_panic(expected = "each stakeholder must have at least 1 bps")]
fn test_configure_stakeholders_zero_share_panics() {
    let (env, client, admin, _attestation, _token) = setup();
    let stakeholders = stakeholders_with_shares(&env, &[10_000, 0]);

    client.configure_stakeholders(&admin_nonce(&client, &admin), &stakeholders);
}

#[test]
#[should_panic(expected = "duplicate stakeholder address")]
fn test_configure_stakeholders_duplicate_address_panics() {
    let (env, client, admin, _attestation, _token) = setup();
    let duplicated = Address::generate(&env);
    let mut stakeholders = Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: duplicated.clone(),
        share_bps: 5000,
    });
    stakeholders.push_back(Stakeholder {
        address: duplicated,
        share_bps: 5000,
    });

    client.configure_stakeholders(&admin_nonce(&client, &admin), &stakeholders);
}

// ════════════════════════════════════════════════════════════════════
//  Distribution Invariant Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_distribute_revenue_exact_split_records_amounts_and_timestamp() {
    let (env, client, admin, _attestation, token) = setup();
    let stakeholders = stakeholders_with_shares(&env, &[6000, 4000]);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 10_000);

    env.ledger().set_timestamp(1_717_171_717);
    let period = String::from_str(&env, "2026-02");
    client.distribute_revenue(&business, &period, &10_000);

    assert_distribution_invariants(
        &env,
        &client,
        &token,
        &business,
        &period,
        &stakeholders,
        10_000,
    );

    let record = client.get_distribution(&business, &period).unwrap();
    assert_eq!(record.timestamp, 1_717_171_717);
    assert_eq!(client.get_distribution_count(&business), 1);
    assert_eq!(token_client.balance(&business), 0);
}

#[test]
fn test_zero_amount_distribution_records_zero_amounts_without_transfers() {
    let (env, client, admin, _attestation, token) = setup();
    let stakeholders = equal_stakeholders(&env, 2);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    client.distribute_revenue(&business, &period, &0);

    let record = client.get_distribution(&business, &period).unwrap();
    assert_eq!(record.total_amount, 0);
    assert_eq!(record.amounts.len(), 2);
    assert_eq!(record.amounts.get(0).unwrap(), 0);
    assert_eq!(record.amounts.get(1).unwrap(), 0);
    assert_eq!(client.get_distribution_count(&business), 1);

    let token_client = TokenClient::new(&env, &token);
    for i in 0..stakeholders.len() {
        let stakeholder = stakeholders.get(i).unwrap();
        assert_eq!(token_client.balance(&stakeholder.address), 0);
    }
}

#[test]
fn test_residual_allocation_prefers_first_stakeholder_even_when_it_has_smallest_share() {
    let (env, client, admin, _attestation, token) = setup();
    let stakeholders = stakeholders_with_shares(&env, &[1, 9_999]);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    mint(&env, &token, &business, 10_001);

    let period = String::from_str(&env, "2026-Q1");
    client.distribute_revenue(&business, &period, &10_001);

    let record = client.get_distribution(&business, &period).unwrap();
    assert_eq!(record.amounts.get(0).unwrap(), 2);
    assert_eq!(record.amounts.get(1).unwrap(), 9_999);
    assert_distribution_invariants(
        &env,
        &client,
        &token,
        &business,
        &period,
        &stakeholders,
        10_001,
    );
}

#[test]
fn test_tiny_revenue_many_stakeholders_allocates_entire_residual_to_first() {
    let (env, client, admin, _attestation, token) = setup();
    let stakeholders = stakeholders_with_shares(&env, &[200; 50]);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    mint(&env, &token, &business, 49);

    let period = String::from_str(&env, "tiny");
    client.distribute_revenue(&business, &period, &49);

    let record = client.get_distribution(&business, &period).unwrap();
    assert_eq!(record.amounts.get(0).unwrap(), 49);
    for i in 1..record.amounts.len() {
        assert_eq!(record.amounts.get(i).unwrap(), 0);
    }
    assert_distribution_invariants(&env, &client, &token, &business, &period, &stakeholders, 49);
}

#[test]
fn test_residual_allocation_invariant_matrix() {
    let configs: [&[u32]; 4] = [
        &[10_000],
        &[5_000, 5_000],
        &[3_334, 3_333, 3_333],
        &[200; 50],
    ];
    let revenues = [0i128, 1, 2, 3, 7, 10, 11, 49, 50, 51, 99, 100, 101, 10_001];

    for shares in configs {
        for revenue in revenues {
            let (env, client, admin, _attestation, token) = setup();
            let stakeholders = stakeholders_with_shares(&env, shares);
            configure_stakeholders_as_admin(&client, &admin, &stakeholders);

            let business = Address::generate(&env);
            mint(&env, &token, &business, revenue);

            let period = String::from_str(&env, "matrix");
            client.distribute_revenue(&business, &period, &revenue);

            assert_distribution_invariants(
                &env,
                &client,
                &token,
                &business,
                &period,
                &stakeholders,
                revenue,
            );
        }
    }
}

#[test]
fn test_duplicate_period_failure_preserves_existing_record_and_balances() {
    let (env, client, admin, _attestation, token) = setup();
    let stakeholders = stakeholders_with_shares(&env, &[6000, 4000]);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 10_000);

    let period = String::from_str(&env, "2026-02");
    client.distribute_revenue(&business, &period, &10_000);

    let record_before = client.get_distribution(&business, &period).unwrap();
    let business_balance_before = token_client.balance(&business);
    let stakeholder1_balance = token_client.balance(&stakeholders.get(0).unwrap().address);
    let stakeholder2_balance = token_client.balance(&stakeholders.get(1).unwrap().address);

    assert!(client
        .try_distribute_revenue(&business, &period, &10_000)
        .is_err());

    let record_after = client.get_distribution(&business, &period).unwrap();
    assert_eq!(record_after, record_before);
    assert_eq!(client.get_distribution_count(&business), 1);
    assert_eq!(token_client.balance(&business), business_balance_before);
    assert_eq!(
        token_client.balance(&stakeholders.get(0).unwrap().address),
        stakeholder1_balance
    );
    assert_eq!(
        token_client.balance(&stakeholders.get(1).unwrap().address),
        stakeholder2_balance
    );
}

#[test]
fn test_failed_transfer_reverts_distribution_state_and_transfers() {
    let (env, client, admin, _attestation, token) = setup();
    let stakeholders = stakeholders_with_shares(&env, &[6000, 4000]);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 9_999);

    let period = String::from_str(&env, "2026-02");
    assert!(client
        .try_distribute_revenue(&business, &period, &10_000)
        .is_err());

    assert!(client.get_distribution(&business, &period).is_none());
    assert_eq!(client.get_distribution_count(&business), 0);
    assert_eq!(token_client.balance(&business), 9_999);
    assert_eq!(
        token_client.balance(&stakeholders.get(0).unwrap().address),
        0
    );
    assert_eq!(
        token_client.balance(&stakeholders.get(1).unwrap().address),
        0
    );
}

#[test]
fn test_reconfiguring_stakeholders_only_affects_future_distributions() {
    let (env, client, admin, _attestation, token) = setup();

    let stakeholders_v1 = stakeholders_with_shares(&env, &[6000, 4000]);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders_v1);

    let business = Address::generate(&env);
    mint(&env, &token, &business, 20_000);

    let first_period = String::from_str(&env, "2026-01");
    client.distribute_revenue(&business, &first_period, &10_000);
    let first_record = client.get_distribution(&business, &first_period).unwrap();

    let stakeholders_v2 = stakeholders_with_shares(&env, &[5000, 3000, 2000]);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders_v2);

    let second_period = String::from_str(&env, "2026-02");
    client.distribute_revenue(&business, &second_period, &10_000);

    assert_eq!(
        client.get_distribution(&business, &first_period).unwrap(),
        first_record
    );
    assert_distribution_invariants(
        &env,
        &client,
        &token,
        &business,
        &second_period,
        &stakeholders_v2,
        10_000,
    );
    assert_eq!(client.get_distribution_count(&business), 2);
}

#[test]
fn test_multiple_periods_increment_count_only_for_successful_distributions() {
    let (env, client, admin, _attestation, token) = setup();
    let stakeholders = equal_stakeholders(&env, 2);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    mint(&env, &token, &business, 20_000);

    let jan = String::from_str(&env, "2026-01");
    let feb = String::from_str(&env, "2026-02");

    client.distribute_revenue(&business, &jan, &10_000);
    assert!(client
        .try_distribute_revenue(&business, &jan, &10_000)
        .is_err());
    client.distribute_revenue(&business, &feb, &10_000);

    assert_eq!(client.get_distribution_count(&business), 2);
    assert!(client.get_distribution(&business, &jan).is_some());
    assert!(client.get_distribution(&business, &feb).is_some());
}

#[test]
#[should_panic(expected = "stakeholders not configured")]
fn test_distribute_revenue_no_stakeholders_panics() {
    let (env, client, _admin, _attestation, _token) = setup();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");

    client.distribute_revenue(&business, &period, &10_000);
}

#[test]
#[should_panic(expected = "revenue amount must be non-negative")]
fn test_distribute_revenue_negative_amount_panics() {
    let (env, client, admin, _attestation, _token) = setup();
    let stakeholders = equal_stakeholders(&env, 2);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    client.distribute_revenue(&business, &period, &-1);
}

// ════════════════════════════════════════════════════════════════════
//  Share Calculation Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_calculate_share_edge_cases() {
    assert_eq!(RevenueShareContract::calculate_share(0, 5_000), 0);
    assert_eq!(
        RevenueShareContract::calculate_share(10_000, 10_000),
        10_000
    );
    assert_eq!(RevenueShareContract::calculate_share(10_000, 1), 1);
    assert_eq!(RevenueShareContract::calculate_share(10_001, 3_333), 3_333);
    assert_eq!(
        RevenueShareContract::calculate_share(1_000_000_000, 5_000),
        500_000_000
    );
}
