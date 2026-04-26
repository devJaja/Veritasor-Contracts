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

fn distribute_nonce(client: &RevenueShareContractClient<'_>, business: &Address) -> u64 {
    client.get_replay_nonce(business, &NONCE_CHANNEL_DISTRIBUTE)
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

fn equal_stakeholders(env: &Env, count: u32) -> soroban_sdk::Vec<Stakeholder> {
    create_stakeholders(env, count, true)
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

fn stakeholders_with_shares(env: &Env, shares: &[u32]) -> soroban_sdk::Vec<Stakeholder> {
    let mut stakeholders = soroban_sdk::Vec::new(env);
    for share in shares {
        stakeholders.push_back(Stakeholder {
            address: Address::generate(env),
            share_bps: *share,
        });
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

    assert_eq!(admin_nonce(&client, &admin), n + 1);
    assert_eq!(client.get_stakeholders().unwrap(), stakeholders);
}

#[test]
fn test_configure_stakeholders_custom_split() {
    let (env, client, _att, admin, _token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, false);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    assert_eq!(admin_nonce(&client, &admin), n + 1);
    assert_eq!(client.get_stakeholders().unwrap(), stakeholders);
}

#[test]
fn test_configure_stakeholders_three_way() {
    let (env, client, _att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 3, false);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    assert_eq!(admin_nonce(&client, &admin), n + 1);
    assert_eq!(client.get_stakeholders().unwrap(), stakeholders);

    let new_token = Address::generate(&env);
    let nonce_before = admin_nonce(&client, &admin);
    set_token_as_admin(&client, &admin, &new_token);
    assert_eq!(client.get_token(), new_token);
    assert_eq!(admin_nonce(&client, &admin), nonce_before + 1);
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

    let duplicated = Address::generate(&env);
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
    let (env, client, att, admin, token) = setup();
    let stakeholders = equal_stakeholders(&env, 2);
    configure_stakeholders_as_admin(&client, &admin, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    submit_revenue_attestation(&env, &att, &business, &period, 0, None);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &0, &dn);

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
    // With 5000, 3000, 2000 bps: should get 50000, 30000, 20000
    assert_eq!(record.total_amount, 100_000);
    assert_eq!(token_client.balance(&stakeholder1.address), 50_000);
    assert_eq!(token_client.balance(&stakeholder2.address), 30_000);
    assert_eq!(token_client.balance(&stakeholder3.address), 20_000);
    assert_distribution_invariants(&env, &client, &token, &business, &period, &stakeholders, 100_000);
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

// ════════════════════════════════════════════════════════════════════
//  Rounding Dust Handling - Deterministic Edge Cases
// ════════════════════════════════════════════════════════════════════
//
// The revenue share contract uses deterministic dust allocation to ensure
// that rounding residuals (dust) are always assigned to the first stakeholder.
// This guarantees:
// 1. Total distributed always equals revenue_amount (no loss)
// 2. Residual is always < number_of_stakeholders (maximum 1 unit per stakeholder attempt)
// 3. First stakeholder receives base share + any accumulated residual
// 4.  Other stakeholders receive exactly their calculated base shares
//
// Testing focus: Prime numbers, odd recipient counts, non-divisible revenues,
// and edge cases that maximize rounding artifacts.

#[test]
fn test_rounding_dust_three_stakeholders_odd_prime() {
    let (env, client, att, admin, token, _att_id) = setup();

    // Create stakeholders with odd prime-based shares: 3331, 3333, 3336 (sum=10000)
    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3331,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3333,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3336,
    });
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-01");
    let revenue = 10_007i128; // Prime number

    submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    assert_distribution_invariants(&env, &client, &token, &business, &period, &stakeholders, revenue);
}

#[test]
fn test_rounding_dust_five_stakeholders_equal_shares() {
    let (env, client, att, admin, token, _att_id) = setup();

    // 5 stakeholders with equal 2000 bps each
    let stakeholders = create_stakeholders(&env, 5, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let revenue = 12_345i128; // Non-divisible by 5

   submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    assert_distribution_invariants(&env, &client, &token, &business, &period, &stakeholders, revenue);
}

#[test]
fn test_rounding_dust_seven_stakeholders_varied_shares() {
    let (env, client, att, admin, token, _att_id) = setup();

    // 7 stakeholders with varied shares designed to create rounding
    let mut stakeholders = soroban_sdk::Vec::new(&env);
    for _ in 0..7 {
        stakeholders.push_back(Stakeholder {
            address: Address::generate(&env),
            share_bps: if stakeholders.len() == 6 {
                10_000 - (6 * 1_428) // Adjust last to sum to 10_000
            } else {
                1_428
            },
        });
    }
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-03");
    let revenue = 99_999i128; // Large non-divisible

    submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    assert_distribution_invariants(&env, &client, &token, &business, &period, &stakeholders, revenue);
}

#[test]
fn test_rounding_dust_small_revenue_many_stakeholders() {
    let (env, client, att, admin, token, _att_id) = setup();

    // 10 stakeholders, small revenue to ensure most get 0
    let stakeholders = create_stakeholders(&env, 10, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-04");
    let revenue = 7i128; // Tiny: only first gets residual

    submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    let record = client.get_distribution(&business, &period).unwrap();
    let first_stakeholder = stakeholders.get(0).unwrap();
    let token_client = TokenClient::new(&env, &token);

    // First stakeholder gets the full residual
    assert_eq!(token_client.balance(&first_stakeholder.address), revenue);

    // All others get 0
    for i in 1..stakeholders.len() {
        let stakeholder = stakeholders.get(i).unwrap();
        assert_eq!(token_client.balance(&stakeholder.address), 0);
    }

    // Verify total
    assert_eq!(sum_amounts(&record.amounts), revenue);
}

#[test]
fn test_rounding_dust_first_stakeholder_always_gets_residual() {
    let (env, client, att, admin, token, _att_id) = setup();

    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3333,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3333,
    });
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 3334,
    });
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-05");
    let revenue = 10_000i128;

    submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    let record = client.get_distribution(&business, &period).unwrap();
    let token_client = TokenClient::new(&env, &token);

    // Calculate expected base amounts
    let base1 = RevenueShareContract::calculate_share(revenue, 3333); // 3333
    let base2 = RevenueShareContract::calculate_share(revenue, 3333); // 3333
    let base3 = RevenueShareContract::calculate_share(revenue, 3334); // 3334
    let residual = revenue - base1 - base2 - base3; // Residual dust

    let first_stakeholder = stakeholders.get(0).unwrap();
    let second_stakeholder = stakeholders.get(1).unwrap();
    let third_stakeholder = stakeholders.get(2).unwrap();

    // First stakeholder gets base + residual
    assert_eq!(token_client.balance(&first_stakeholder.address), base1 + residual);
    // Others get exactly base
    assert_eq!(token_client.balance(&second_stakeholder.address), base2);
    assert_eq!(token_client.balance(&third_stakeholder.address), base3);

    // Verify in record
    assert_eq!(record.amounts.get(0).unwrap(), base1 + residual);
    assert_eq!(record.amounts.get(1).unwrap(), base2);
    assert_eq!(record.amounts.get(2).unwrap(), base3);
}

#[test]
fn test_rounding_dust_50_stakeholders_max_count() {
    let (env, client, att, admin, token, _att_id) = setup();

    // Maximum 50 stakeholders with minimal 200 bps each (10000/50 = 200)
    let stakeholders = create_stakeholders(&env, 50, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-06");
    let revenue = 999_999i128; // Large, non-divisible

    submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    assert_distribution_invariants(&env, &client, &token, &business, &period, &stakeholders, revenue);
}

#[test]
fn test_rounding_dust_two_stakeholders_6000_4000_split() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, false); // 6000, 4000
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-07");
    let revenue = 13i128;  // 13 * 0.6 = 7.8 -> 7, 13 * 0.4 = 5.2 -> 5, residual = 1

    submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    let record = client.get_distribution(&business, &period).unwrap();
    let token_client = TokenClient::new(&env, &token);

    let s1 = stakeholders.get(0).unwrap();
    let s2 = stakeholders.get(1).unwrap();

    // 13 * 6000 / 10000 = 7.8 -> 7 (base)
    // 13 * 4000 / 10000 = 5.2 -> 5 (base)
    // Residual: 13 - 7 - 5 = 1
    // First gets 7 + 1 = 8, second gets 5
    assert_eq!(token_client.balance(&s1.address), 8);
    assert_eq!(token_client.balance(&s2.address), 5);
    assert_eq!(sum_amounts(&record.amounts), revenue);
}

#[test]
fn test_rounding_dust_consistency_multiple_distributions() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 3, false);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    mint(&env, &token, &business, 300_000);

    let token_client = TokenClient::new(&env, &token);

    // Execute 3 distributions with same revenue to verify determinism
    for dist_idx in 0..3 {
        let period = String::from_str(&env, &format!("2026-period-{}", dist_idx));
        let revenue = 10_001i128;

        submit_revenue_attestation(&env, &att, &business, &period, revenue, None);

        let dn = distribute_nonce(&client, &business);
        client.distribute_revenue(&business, &period, &revenue, &dn);

        let record = client.get_distribution(&business, &period).unwrap();
        assert_eq!(sum_amounts(&record.amounts), revenue);

        // Distributions are independent per period; verify record exists
        assert_eq!(record.total_amount, revenue);
    }

    assert_eq!(client.get_distribution_count(&business), 3);
}

#[test]
fn test_rounding_dust_max_i128_revenue_safe() {
    let (env, client, att, admin, token, _att_id) = setup();

    let stakeholders = create_stakeholders(&env, 2, true);
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-max");
    // Use a very large i128 value (but not i128::MAX to avoid overflow in test setup)
    let revenue = 9_223_372_036_854_775_000i128;

    submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    let record = client.get_distribution(&business, &period).unwrap();
    assert_eq!(sum_amounts(&record.amounts), revenue);
    assert_distribution_invariants(&env, &client, &token, &business, &period, &stakeholders, revenue);
}

#[test]
fn test_rounding_dust_single_stakeholder_no_residual() {
    let (env, client, att, admin, token, _att_id) = setup();

    let mut stakeholders = soroban_sdk::Vec::new(&env);
    stakeholders.push_back(Stakeholder {
        address: Address::generate(&env),
        share_bps: 10_000,
    });
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-single");
    let revenue = 500_000i128;

    submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
    mint(&env, &token, &business, revenue);

    let dn = distribute_nonce(&client, &business);
    client.distribute_revenue(&business, &period, &revenue, &dn);

    let record = client.get_distribution(&business, &period).unwrap();
    let stakeholder = stakeholders.get(0).unwrap();
    let token_client = TokenClient::new(&env, &token);

    // Single stakeholder gets 100%, no residual
    assert_eq!(token_client.balance(&stakeholder.address), revenue);
    assert_eq!(record.amounts.get(0).unwrap(), revenue);
    assert_eq!(sum_amounts(&record.amounts), revenue);
}

#[test]
fn test_rounding_dust_third_stakeholder_assignment_never_happens() {
    let (env, client, att, admin, token, _att_id) = setup();

    // 3 stakeholders: residual should never go to 2nd or 3rd
    let mut stakeholders = soroban_sdk::Vec::new(&env);
    for i in 0..3 {
        stakeholders.push_back(Stakeholder {
            address: Address::generate(&env),
            share_bps: if i == 2 { 3334 } else { 3333 },
        });
    }
    let n = admin_nonce(&client, &admin);
    client.configure_stakeholders(&n, &stakeholders);

    // Test multiple revenues to find rounding patterns
    let revenues = vec![7i128, 13i128, 100i128, 1_000i128, 3_333i128, 10_000i128, 99_999i128];

    for (idx, &revenue) in revenues.iter().enumerate() {
        let business = Address::generate(&env);
        let period = String::from_str(&env, &format!("2026-check-{}", idx));

        submit_revenue_attestation(&env, &att, &business, &period, revenue, None);
        mint(&env, &token, &business, revenue);

        let dn = distribute_nonce(&client, &business);
        client.distribute_revenue(&business, &period, &revenue, &dn);

        let record = client.get_distribution(&business, &period).unwrap();
        let token_client = TokenClient::new(&env, &token);

        let base0 = RevenueShareContract::calculate_share(revenue, 3333);
        let base1 = RevenueShareContract::calculate_share(revenue, 3333);
        let base2 = RevenueShareContract::calculate_share(revenue, 3334);

        let s0 = stakeholders.get(0).unwrap();
        let s1 = stakeholders.get(1).unwrap();
        let s2 = stakeholders.get(2).unwrap();

        let actual0 = token_client.balance(&s0.address);
        let actual1 = token_client.balance(&s1.address);
        let actual2 = token_client.balance(&s2.address);

        // Verify residual only goes to first
        assert_eq!(actual0, base0 + (revenue - base0 - base1 - base2));
        assert_eq!(actual1, base1);
        assert_eq!(actual2, base2);

        // Verify sum
        assert_eq!(actual0 + actual1 + actual2, revenue);
    }
}
