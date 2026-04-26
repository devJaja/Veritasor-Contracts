#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env, String};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> token::StellarAssetClient<'a> {
    token::StellarAssetClient::new(
        env,
        &env.register_stellar_asset_contract_v2(admin.clone()).address(),
    )
}

fn setup_test() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let owner = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token = token_client.address.clone();
    token_client.mint(&issuer, &100_000_000);
    let attestation_contract = Address::generate(&env);
    (env, admin, issuer, owner, token, attestation_contract)
}

fn set_mock_revenue(env: &Env, business: &Address, period: &str, revenue: i128) {
    let p = String::from_str(env, period);
    env.storage().temporary().set(
        &(soroban_sdk::symbol_short!("rev"), business.clone(), p),
        &revenue,
    );
}

// ─── parse_period ─────────────────────────────────────────────────────────────

#[test]
fn test_parse_period_valid() {
    let env = Env::default();
    let p = String::from_str(&env, "2026-02");
    assert_eq!(parse_period(&env, p), 2026u64 * 12 + 1);
}

#[test]
#[should_panic(expected = "invalid period length")]
fn test_parse_period_invalid_length() {
    let env = Env::default();
    parse_period(&env, String::from_str(&env, "2026-2"));
}

#[test]
#[should_panic(expected = "invalid year digit")]
fn test_parse_period_invalid_digit() {
    let env = Env::default();
    parse_period(&env, String::from_str(&env, "202a-02"));
}

// ─── is_period_within_maturity ────────────────────────────────────────────────

fn make_bond(env: &Env, issue_period: &str, maturity_periods: u32) -> Bond {
    let dummy = Address::generate(env);
    Bond {
        id: 0,
        issuer: dummy.clone(),
        face_value: 1_000_000,
        structure: BondStructure::Fixed,
        revenue_share_bps: 0,
        min_payment_per_period: 100_000,
        max_payment_per_period: 100_000,
        maturity_periods,
        attestation_contract: dummy.clone(),
        token: dummy,
        status: BondStatus::Active,
        issued_at: 0,
        issue_period: String::from_str(env, issue_period),
    }
}

#[test]
fn test_is_within_maturity_valid() {
    let env = Env::default();
    let bond = make_bond(&env, "2026-01", 12);
    assert!(is_period_within_maturity(&env, &bond, String::from_str(&env, "2026-12")));
}

#[test]
fn test_is_within_maturity_expired() {
    let env = Env::default();
    let bond = make_bond(&env, "2026-01", 12);
    assert!(!is_period_within_maturity(&env, &bond, String::from_str(&env, "2027-01")));
}

// ─── Maturity enforcement via contract ───────────────────────────────────────

#[test]
fn test_redeem_within_maturity() {
    let (env, admin, issuer, owner, token, attestation_contract) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let bond_id = client.issue_bond(
        &issuer, &owner, &1_000_000, &BondStructure::Fixed,
        &0, &100_000, &100_000, &12,
        &String::from_str(&env, "2026-01"),
        &attestation_contract, &token,
    );

    set_mock_revenue(&env, &issuer, "2026-02", 0);
    client.redeem(&bond_id, &String::from_str(&env, "2026-02"));
}

#[test]
#[should_panic(expected = "period exceeds maturity")]
fn test_redeem_post_maturity_panics() {
    let (env, admin, issuer, owner, token, attestation_contract) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let bond_id = client.issue_bond(
        &issuer, &owner, &1_000_000, &BondStructure::Fixed,
        &0, &100_000, &100_000, &1,
        &String::from_str(&env, "2026-01"),
        &attestation_contract, &token,
    );

    // maturity_periods=1 → only "2026-01" is valid; "2026-02" is past maturity.
    set_mock_revenue(&env, &issuer, "2026-02", 0);
    client.redeem(&bond_id, &String::from_str(&env, "2026-02"));
}

#[test]
#[should_panic(expected = "bond not active")]
fn test_redeem_matured_bond_panics() {
    let (env, admin, issuer, owner, token, attestation_contract) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let bond_id = client.issue_bond(
        &issuer, &owner, &1_000_000, &BondStructure::Fixed,
        &0, &100_000, &100_000, &12,
        &String::from_str(&env, "2026-01"),
        &attestation_contract, &token,
    );

    client.mark_matured(&admin, &bond_id);
    set_mock_revenue(&env, &issuer, "2026-02", 0);
    client.redeem(&bond_id, &String::from_str(&env, "2026-02"));
}

#[test]
fn test_remaining_value_matured_is_zero() {
    let (env, admin, issuer, owner, token, attestation_contract) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let bond_id = client.issue_bond(
        &issuer, &owner, &1_000_000, &BondStructure::Fixed,
        &0, &100_000, &100_000, &12,
        &String::from_str(&env, "2026-01"),
        &attestation_contract, &token,
    );

    client.mark_matured(&admin, &bond_id);
    assert_eq!(client.get_remaining_value(&bond_id), 0);
}
