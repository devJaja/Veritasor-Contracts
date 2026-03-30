#![cfg(test)]

use super::{
    RevenueShareContract, RevenueShareContractClient, Stakeholder, NONCE_CHANNEL_ADMIN,
    NONCE_CHANNEL_DISTRIBUTE, MAX_PERIOD_BYTES,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{Address, Bytes, BytesN, Env, String};
use veritasor_attestation::{AttestationContract, AttestationContractClient};

// ════════════════════════════════════════════════════════════════════
//  Test Helpers
// ════════════════════════════════════════════════════════════════════

/// Full stack: attestation + revenue-share + USDC-style token.
fn setup() -> (
    Env,
    RevenueShareContractClient<'static>,
    AttestationContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let attestation_id = env.register(AttestationContract, ());
    let att_client = AttestationContractClient::new(&env, &attestation_id);
    att_client.initialize(&admin, &0u64);
    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    att_client.configure_fees(&fee_token, &collector, &0i128, &false);

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token = sac.address();

    let rev_id = env.register(RevenueShareContract, ());
    let client = RevenueShareContractClient::new(&env, &rev_id);
    client.initialize(&admin, &0u64, &attestation_id, &token);

    (env, client, att_client, admin, token, attestation_id)
}

/// Merkle root commitment for a scalar revenue amount (matches contract + lender pattern).
fn revenue_merkle_root(env: &Env, revenue: i128) -> BytesN<32> {
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&revenue.to_be_bytes());
    let payload = Bytes::from_slice(env, &buf);
    env.crypto().sha256(&payload).into()
}

fn submit_revenue_attestation(
    env: &Env,
    att: &AttestationContractClient<'_>,
    business: &Address,
    period: &String,
    revenue: i128,
    expiry: Option<u64>,
) {
    let root = revenue_merkle_root(env, revenue);
    att.submit_attestation(
        business,
        period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &expiry,
    );
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

fn admin_nonce(client: &RevenueShareContractClient<'_>, admin: &Address) -> u64 {
    client.get_replay_nonce(admin, &NONCE_CHANNEL_ADMIN)
}

fn distribute_nonce(client: &RevenueShareContractClient<'_>, business: &Address) -> u64 {
    client.get_replay_nonce(business, &NONCE_CHANNEL_DISTRIBUTE)
}

fn create_stakeholders(env: &Env, count: u32, equal_shares: bool) -> soroban_sdk::Vec<Stakeholder> {
    let mut stakeholders = soroban_sdk::Vec::new(env);

    if equal_shares {
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
    } else {
        match count {
            2 => {
                stakeholders.push_back(Stakeholder {
                    address: Address::generate(env),
                    share_bps: 6000,
                });
                stakeholders.push_back(Stakeholder {
                    address: Address::generate(env),
                    share_bps: 4000,
                });
            }
            3 => {
                stakeholders.push_back(Stakeholder {
                    address: Address::generate(env),
                    share_bps: 5000,
                });
                stakeholders.push_back(Stakeholder {
                    address: Address::generate(env),
                    share_bps: 3000,
                });
                stakeholders.push_back(Stakeholder {
                    address: Address::generate(env),
                    share_bps: 2000,
                });
            }
            _ => panic!("unsupported count for non-equal shares"),
        }
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
//  Initialization & Constants
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_initialize() {
    let (env, client, _att, admin, token, att_id) = setup();

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_attestation_contract(), att_id);
    assert_eq!(client.get_max_period_bytes(), MAX_PERIOD_BYTES);
    assert_eq!(client.get_token(), token);
    let _ = env;
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_twice_panics() {
    let (env, client, _att, admin, token, att_id) = setup();
    let new_admin = Address::generate(&env);
    client.initialize(&new_admin, &0u64, &att_id, &token);
}

// ════════════════════════════════════════════════════════════════════
//  Stakeholder Configuration
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_configure_stakeholders_two_equal() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    assert!(client
        .try_configure_stakeholders(&nonce, &stakeholders)
        .is_err());
    assert_eq!(admin_nonce(&client, &admin), nonce + 1);
    assert_eq!(client.get_stakeholders().unwrap(), stakeholders);
}

#[test]
fn test_configure_stakeholders_custom_split() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, false);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    client.configure_stakeholders(&nonce, &valid);
    assert_eq!(admin_nonce(&client, &admin), nonce + 1);
    assert_eq!(client.get_stakeholders().unwrap(), valid);
}

#[test]
fn test_configure_stakeholders_three_way() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 3, false);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    set_token_as_admin(&client, &admin, &new_token);
    assert_eq!(client.get_token(), new_token);
    assert_eq!(admin_nonce(&client, &admin), nonce_before + 2);
}

// ════════════════════════════════════════════════════════════════════
//  Stakeholder Configuration Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_configure_stakeholders_many() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 10, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let updated = stakeholders_with_shares(&env, &[5000, 3000, 2000]);
    configure_stakeholders_as_admin(&client, &admin, &updated);
    assert_eq!(client.get_stakeholders().unwrap(), updated);
}

#[test]
#[should_panic(expected = "must have at least one stakeholder")]
fn test_configure_stakeholders_empty_panics() {
    let (env, client, _att, admin, _token, _att_id) = setup();
    let stakeholders = soroban_sdk::Vec::new(&env);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);
}

#[test]
#[should_panic(expected = "cannot exceed 50 stakeholders")]
fn test_configure_stakeholders_too_many_panics() {
    let (env, client, _att, admin, _token, _att_id) = setup();
    let stakeholders = create_stakeholders(&env, 51, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);
}

#[test]
#[should_panic(expected = "total shares must equal 10,000 bps (100%)")]
fn test_configure_stakeholders_invalid_total_panics() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 5000,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 4000,
    });

    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);
}

#[test]
#[should_panic(expected = "each stakeholder must have at least 1 bps")]
fn test_configure_stakeholders_zero_share_panics() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 10_000,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 0,
    });

    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);
}

#[test]
#[should_panic(expected = "duplicate stakeholder address")]
fn test_configure_stakeholders_duplicate_address_panics() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let addr = Address::generate(&env);
    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: duplicated.clone(),
        share_bps: 5000,
    });
    stakeholders.push_back(Stakeholder {
        address: duplicated,
        share_bps: 5000,
    });

    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);
}

#[test]
#[should_panic(expected = "nonce mismatch")]
fn test_configure_stakeholders_wrong_admin_nonce_panics() {
    let (env, client, _att, admin, _token, _att_id) = setup();
    let stakeholders = create_stakeholders(&env, 2, true);
    client.configure_stakeholders(&0u64, &stakeholders);
}

// ════════════════════════════════════════════════════════════════════
//  Distribution (with attestation + nonces)
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_distribute_revenue_two_stakeholders() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, false);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 10_000, None);

    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 10_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &dn);

    let stakeholder1 = stakeholders.get(0).unwrap();
    let stakeholder2 = stakeholders.get(1).unwrap();

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
fn test_distribute_revenue_three_stakeholders() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 3, false);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    submit_revenue_attestation(&env, &att, &business, &period, 100_000, None);

    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 100_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &100_000, &dn);

    let stakeholder1 = stakeholders.get(0).unwrap();
    let stakeholder2 = stakeholders.get(1).unwrap();
    let stakeholder3 = stakeholders.get(2).unwrap();

    let record = client.get_distribution(&business, &period).unwrap();
    assert_eq!(record.amounts.get(0).unwrap(), 49);
    for i in 1..record.amounts.len() {
        assert_eq!(record.amounts.get(i).unwrap(), 0);
    }
    assert_distribution_invariants(&env, &client, &token, &business, &period, &stakeholders, 49);
}

#[test]
fn test_distribute_revenue_with_rounding() {
    let (env, client, att, admin, token, _att_id) = setup();

    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3334,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3333,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3333,
    });
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 10_000, None);

    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 10_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &dn);

    let stakeholder1 = stakeholders.get(0).unwrap();
    let stakeholder2 = stakeholders.get(1).unwrap();
    let stakeholder3 = stakeholders.get(2).unwrap();

    let bal1 = token_client.balance(&stakeholder1.address);
    let bal2 = token_client.balance(&stakeholder2.address);
    let bal3 = token_client.balance(&stakeholder3.address);

    assert_eq!(bal1 + bal2 + bal3, 10_000);
    assert!(bal1 >= bal2);
    assert!(bal1 >= bal3);
}

#[test]
fn test_distribute_revenue_zero_amount() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 0, None);

    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 9_999);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &0, &dn);

    let stakeholder1 = stakeholders.get(0).unwrap();
    let stakeholder2 = stakeholders.get(1).unwrap();
    assert_eq!(token_client.balance(&stakeholder1.address), 0);
    assert_eq!(token_client.balance(&stakeholder2.address), 0);

    let record = client.get_distribution(&business, &period).unwrap();
    assert_eq!(record.total_amount, 0);
}

#[test]
fn test_distribute_revenue_multiple_periods_increments_nonces() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    mint(&env, &token, &business, 20_000);

    let p1 = String::from_str(&env, "2026-01");
    submit_revenue_attestation(&env, &att, &business, &p1, 10_000, None);
    let d1 = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &p1, &10_000, &d1);

    let p2 = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &p2, 10_000, None);
    let d2 = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &p2, &10_000, &d2);

    let p3 = String::from_str(&env, "2026-03");
    submit_revenue_attestation(&env, &att, &business, &p3, 10_000, None);
    let d3 = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &p3, &10_000, &d3);

    assert_eq!(client.get_distribution_count(&business), 3);
    assert_eq!(client.get_replay_nonce(&business, &NONCE_CHANNEL_DISTRIBUTE), 3);
}

#[test]
#[should_panic(expected = "distribution already executed for this period")]
fn test_distribute_revenue_duplicate_period_panics() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 10_000, None);
    mint(&env, &token, &business, 20_000);

    let d1 = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &d1);
    let d2 = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &d2);
}

#[test]
#[should_panic(expected = "nonce mismatch")]
fn test_distribute_revenue_reused_nonce_panics() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let p1 = String::from_str(&env, "2026-01");
    let p2 = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &p1, 10_000, None);
    submit_revenue_attestation(&env, &att, &business, &p2, 10_000, None);
    mint(&env, &token, &business, 20_000);

    let d = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &p1, &10_000, &d);
    client.distribute_revenue(&business, &p2, &10_000, &d);
}

#[test]
#[should_panic(expected = "stakeholders not configured")]
fn test_distribute_revenue_no_stakeholders_panics() {
    let (env, client, att, _admin, token, _att_id) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 10_000, None);
    mint(&env, &token, &business, 10_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &dn);
}

#[test]
#[should_panic(expected = "revenue amount must be non-negative")]
fn test_distribute_revenue_negative_amount_panics() {
    let (env, client, att, admin, _token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &-1000, &dn);
}

#[test]
#[should_panic(expected = "attestation not found")]
fn test_distribute_without_attestation_panics() {
    let (env, client, _att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    mint(&env, &token, &business, 10_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &dn);
}

#[test]
#[should_panic(expected = "revenue amount does not match attested merkle root")]
fn test_distribute_wrong_amount_vs_attestation_panics() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 10_000, None);
    mint(&env, &token, &business, 10_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &9_999, &dn);
}

#[test]
#[should_panic(expected = "insufficient token balance for distribution")]
fn test_distribute_insufficient_balance_panics() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 10_000, None);
    mint(&env, &token, &business, 9_999);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &dn);
}

#[test]
#[should_panic(expected = "attestation expired")]
fn test_distribute_expired_attestation_panics() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    env.ledger().set_timestamp(1_000);
    submit_revenue_attestation(&env, &att, &business, &period, 10_000, Some(1_500));
    env.ledger().set_timestamp(2_000);
    mint(&env, &token, &business, 10_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &dn);
}

#[test]
#[should_panic(expected = "period exceeds maximum length")]
fn test_distribute_period_too_long_panics() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let long = core::str::from_utf8(&[b'x'; (MAX_PERIOD_BYTES as usize) + 1]).unwrap();
    let period = String::from_str(&env, long);

    let business = Address::generate(&env);
    submit_revenue_attestation(&env, &att, &business, &period, 10_000, None);
    mint(&env, &token, &business, 10_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &10_000, &dn);
}

#[test]
fn test_period_at_max_length_succeeds() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let long = core::str::from_utf8(&[b'y'; MAX_PERIOD_BYTES as usize]).unwrap();
    let period = String::from_str(&env, long);

    let business = Address::generate(&env);
    submit_revenue_attestation(&env, &att, &business, &period, 100, None);
    mint(&env, &token, &business, 100);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &100, &dn);

    assert!(client.get_distribution(&business, &period).is_some());
}

// ════════════════════════════════════════════════════════════════════
//  Share calculation
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_calculate_share_exact() {
    assert_eq!(RevenueShareContract::calculate_share(10_000, 5000), 5_000);
    assert_eq!(RevenueShareContract::calculate_share(10_000, 2500), 2_500);
    assert_eq!(RevenueShareContract::calculate_share(100_000, 1000), 10_000);
}

#[test]
fn test_calculate_share_rounding() {
    assert_eq!(RevenueShareContract::calculate_share(10_000, 3333), 3_333);
    assert_eq!(RevenueShareContract::calculate_share(1_000, 3333), 333);
}

#[test]
fn test_calculate_share_edge_cases() {
    assert_eq!(RevenueShareContract::calculate_share(0, 5000), 0);
    assert_eq!(
        RevenueShareContract::calculate_share(10_000, 10_000),
        10_000
    );
    assert_eq!(RevenueShareContract::calculate_share(10_000, 1), 1);
    assert_eq!(
        RevenueShareContract::calculate_share(1_000_000_000, 5_000),
        500_000_000
    );
}

#[test]
#[should_panic(expected = "calculate_share overflow")]
fn test_calculate_share_overflow_panics() {
    let _ = RevenueShareContract::calculate_share(i128::MAX, 10_001);
}

// ════════════════════════════════════════════════════════════════════
//  Extreme allocations
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_extreme_allocation_one_stakeholder_100_percent() {
    let (env, client, att, admin, token, _att_id) = setup();

    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 10_000,
    });
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 100_000, None);

    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 100_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &100_000, &dn);

    let stakeholder = stakeholders.get(0).unwrap();
    assert_eq!(token_client.balance(&stakeholder.address), 100_000);
}

#[test]
fn test_extreme_allocation_99_1_split() {
    let (env, client, att, admin, token, _att_id) = setup();

    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 9_900,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 100,
    });
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 100_000, None);

    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 100_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &100_000, &dn);

    let stakeholder1 = stakeholders.get(0).unwrap();
    let stakeholder2 = stakeholders.get(1).unwrap();

    assert_eq!(token_client.balance(&stakeholder1.address), 99_000);
    assert_eq!(token_client.balance(&stakeholder2.address), 1_000);
}

#[test]
fn test_extreme_allocation_many_small_stakeholders() {
    let (env, client, att, admin, token, _att_id) = setup();

    let mut stakeholders = soroban_sdk::Vec::new(&env);
    for _ in 0..50 {
        stakeholders.push_back(Stakeholder {
            address: Address::generate(&env),
            share_bps: 200,
        });
    }
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 1_000_000, None);

    let token_client = TokenClient::new(&env, &token);
    mint(&env, &token, &business, 1_000_000);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &1_000_000, &dn);

    let mut total = 0i128;
    for i in 0..50 {
        let stakeholder = stakeholders.get(i).unwrap();
        total += token_client.balance(&stakeholder.address);
    }
    assert_eq!(total, 1_000_000);
}

// ════════════════════════════════════════════════════════════════════
//  Configuration updates
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_update_stakeholders() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let stakeholders1 = create_stakeholders(&env, 2, true);
    let n1 = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n1, &stakeholders1);

    let stakeholders2 = create_stakeholders(&env, 3, false);
    let n2 = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n2, &stakeholders2);

    let stored = client.get_stakeholders().unwrap();
    assert_eq!(stored.len(), 3);
}

#[test]
fn test_set_attestation_contract() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let new_attestation = Address::generate(&env);
    let n = admin_nonce(&client, &admin);
    client.set_attestation_contract(&n, &new_attestation);

    assert_eq!(client.get_attestation_contract(), new_attestation);
}

#[test]
fn test_set_token() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let new_token = Address::generate(&env);
    let n = admin_nonce(&client, &admin);
    client.set_token(&n, &new_token);

    assert_eq!(client.get_token(), new_token);
}

// ════════════════════════════════════════════════════════════════════
//  Queries
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_get_distribution_count_zero() {
    let (_env, client, _a, _ad, _t, _aid) = setup();

    let business = Address::generate(&_env);
    assert_eq!(client.get_distribution_count(&business), 0);
}

#[test]
fn test_get_distribution_nonexistent() {
    let (env, client, _a, _ad, _t, _aid) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    assert!(client.get_distribution(&business, &period).is_none());
}

#[test]
fn test_get_stakeholders_not_configured() {
    let (_env, client, _a, _ad, _t, _aid) = setup();
    assert!(client.get_stakeholders().is_none());
}

#[test]
fn test_two_businesses_same_period_independent() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let period = String::from_str(&env, "2026-02");

    let b1 = Address::generate(&env);
    submit_revenue_attestation(&env, &att, &b1, &period, 5_000, None);
    mint(&env, &token, &b1, 5_000);
    let d1 = distribute_nonce(&client, &b1);
    client.distribute_revenue(&b1, &period, &5_000, &d1);

    let b2 = Address::generate(&env);
    submit_revenue_attestation(&env, &att, &b2, &period, 7_000, None);
    mint(&env, &token, &b2, 7_000);
    let d2 = distribute_nonce(&client, &b2);
    client.distribute_revenue(&b2, &period, &7_000, &d2);

    assert_eq!(client.get_distribution(&b1, &period).unwrap().total_amount, 5_000);
    assert_eq!(client.get_distribution(&b2, &period).unwrap().total_amount, 7_000);
}
