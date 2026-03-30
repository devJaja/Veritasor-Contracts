//! Tests for revenue-based lending settlement contract.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env, String};
use veritasor_attestation::{AttestationContract, AttestationContractClient};

fn setup(env: &Env) -> SetupData {
    let admin = Address::generate(env);
    let settlement_contract_id = env.register(RevenueSettlementContract, ());
    let settlement_client = RevenueSettlementContractClient::new(env, &settlement_contract_id);
    settlement_client.initialize(&admin);

    let attestation_id = env.register(AttestationContract, ());
    let attestation_client = AttestationContractClient::new(env, &attestation_id);
    attestation_client.initialize(&admin, &0u64);

    let token_admin = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin);
    let token = token_contract.address().clone();

    let alt_token_admin = Address::generate(env);
    let alt_token_contract = env.register_stellar_asset_contract_v2(alt_token_admin);
    let alt_token = alt_token_contract.address().clone();

    let lender = Address::generate(env);
    let business = Address::generate(env);

    SetupData {
        admin,
        settlement_client,
        attestation_id,
        attestation_client,
        token,
        alt_token,
        lender,
        business,
    }
}

struct SetupData {
    admin: Address,
    settlement_client: RevenueSettlementContractClient<'static>,
    attestation_id: Address,
    attestation_client: AttestationContractClient<'static>,
    token: Address,
    alt_token: Address,
    lender: Address,
    business: Address,
}

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let stored_admin = setup.settlement_client.get_admin();
    assert_eq!(stored_admin, setup.admin);
}

#[test]
fn test_create_agreement() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    assert_eq!(agreement_id, 0);

    let agreement = setup.settlement_client.get_agreement(&agreement_id).unwrap();
    assert_eq!(agreement.id, 0);
    assert_eq!(agreement.lender, setup.lender);
    assert_eq!(agreement.business, setup.business);
    assert_eq!(agreement.principal, principal);
    assert_eq!(agreement.revenue_share_bps, revenue_share_bps);
    assert_eq!(agreement.min_revenue_threshold, min_revenue);
    assert_eq!(agreement.max_repayment_amount, max_repayment);
    assert_eq!(agreement.status, 0);
}

#[test]
#[should_panic(expected = "principal must be positive")]
fn test_create_agreement_invalid_principal() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &0,
        &1000u32,
        &100_000i128,
        &500_000i128,
        &setup.attestation_id,
        &setup.token,
    );
}

#[test]
#[should_panic(expected = "revenue_share_bps must be <= 10000")]
fn test_create_agreement_invalid_share() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &10_000_000i128,
        &10001u32,
        &100_000i128,
        &500_000i128,
        &setup.attestation_id,
        &setup.token,
    );
}

#[test]
fn test_settle_basic_repayment() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);

    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let attested_revenue = 1_000_000i128;
    let business_tokens = 1_500_000i128;

    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &business_tokens);

    setup
        .settlement_client
        .settle(&agreement_id, &period, &attested_revenue);

    let settlement = setup
        .settlement_client
        .get_settlement(&agreement_id, &period)
        .unwrap();

    assert_eq!(settlement.attested_revenue, attested_revenue);
    assert_eq!(settlement.repayment_amount, 100_000i128);
    assert_eq!(settlement.amount_transferred, 100_000i128);
}

#[test]
#[should_panic(expected = "attestation not found")]
fn test_settle_missing_attestation() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let period = String::from_str(&env, "2026-02");
    let attested_revenue = 1_000_000i128;

    setup
        .settlement_client
        .settle(&agreement_id, &period, &attested_revenue);
}

#[test]
#[should_panic(expected = "already settled for period")]
fn test_settle_double_spending_prevention() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);

    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let attested_revenue = 1_000_000i128;
    let business_tokens = 2_000_000i128;

    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &business_tokens);

    setup
        .settlement_client
        .settle(&agreement_id, &period, &attested_revenue);

    setup
        .settlement_client
        .settle(&agreement_id, &period, &attested_revenue);
}

#[test]
fn test_settle_rejects_multi_currency_for_same_business_period() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let primary_agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let second_lender = Address::generate(&env);
    let alternate_agreement_id = setup.settlement_client.create_agreement(
        &second_lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.alt_token,
    );

    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let attested_revenue = 1_000_000i128;
    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &1_500_000i128);
    StellarAssetClient::new(&env, &setup.alt_token).mint(&setup.business, &1_500_000i128);

    setup
        .settlement_client
        .settle(&primary_agreement_id, &period, &attested_revenue);

    let rejected = setup
        .settlement_client
        .try_settle(&alternate_agreement_id, &period, &attested_revenue);
    assert!(rejected.is_err());
    assert!(setup
        .settlement_client
        .get_settlement(&alternate_agreement_id, &period)
        .is_none());
    assert_eq!(
        setup
            .settlement_client
            .get_committed(&alternate_agreement_id, &period),
        0
    );

    let primary = setup
        .settlement_client
        .get_settlement(&primary_agreement_id, &period)
        .unwrap();
    assert_eq!(primary.amount_transferred, 100_000i128);
}

#[test]
fn test_settle_allows_same_currency_across_multiple_agreements() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let first_agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let second_lender = Address::generate(&env);
    let second_agreement_id = setup.settlement_client.create_agreement(
        &second_lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[2u8; 32]);
    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let attested_revenue = 1_000_000i128;
    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &3_000_000i128);

    setup
        .settlement_client
        .settle(&first_agreement_id, &period, &attested_revenue);
    setup
        .settlement_client
        .settle(&second_agreement_id, &period, &attested_revenue);

    assert!(setup
        .settlement_client
        .get_settlement(&first_agreement_id, &period)
        .is_some());
    assert!(setup
        .settlement_client
        .get_settlement(&second_agreement_id, &period)
        .is_some());
}

#[test]
fn test_settle_below_minimum_revenue() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 1_000_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);

    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let attested_revenue = 100_000i128;

    setup
        .settlement_client
        .settle(&agreement_id, &period, &attested_revenue);

    let settlement = setup
        .settlement_client
        .get_settlement(&agreement_id, &period)
        .unwrap();

    assert_eq!(settlement.repayment_amount, 0);
    assert_eq!(settlement.amount_transferred, 0);
}

#[test]
fn test_settle_capped_at_max_repayment() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 50_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);

    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let attested_revenue = 10_000_000i128;
    let business_tokens = 1_000_000i128;

    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &business_tokens);

    setup
        .settlement_client
        .settle(&agreement_id, &period, &attested_revenue);

    let settlement = setup
        .settlement_client
        .get_settlement(&agreement_id, &period)
        .unwrap();

    assert_eq!(settlement.repayment_amount, 50_000i128);
}

#[test]
#[should_panic(expected = "attestation is revoked for period")]
fn test_settle_revoked_attestation() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);

    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let reason = String::from_str(&env, "test revocation");
    setup
        .attestation_client
        .revoke_attestation(&setup.admin, &setup.business, &period, &reason, &0u64);

    let attested_revenue = 1_000_000i128;

    setup
        .settlement_client
        .settle(&agreement_id, &period, &attested_revenue);
}

#[test]
fn test_multiple_periods_settlement() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let business_tokens = 5_000_000i128;
    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &business_tokens);

    let periods = [
        String::from_str(&env, "2026-01"),
        String::from_str(&env, "2026-02"),
        String::from_str(&env, "2026-03"),
    ];

    for (idx, period) in periods.iter().enumerate() {
        let root = soroban_sdk::BytesN::from_array(&env, &[(idx + 1) as u8; 32]);

        setup
            .attestation_client
            .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

        let attested_revenue = 1_000_000i128;
        setup
            .settlement_client
            .settle(&agreement_id, &period, &attested_revenue);

        let settlement = setup
            .settlement_client
            .get_settlement(&agreement_id, &period)
            .unwrap();

        assert_eq!(settlement.repayment_amount, 100_000i128);
    }
}

#[test]
fn test_mark_completed() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let mut agreement = setup.settlement_client.get_agreement(&agreement_id).unwrap();
    assert_eq!(agreement.status, 0);

    setup
        .settlement_client
        .mark_completed(&setup.admin, &agreement_id);

    agreement = setup.settlement_client.get_agreement(&agreement_id).unwrap();
    assert_eq!(agreement.status, 1);
}

#[test]
fn test_mark_defaulted() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let mut agreement = setup.settlement_client.get_agreement(&agreement_id).unwrap();
    assert_eq!(agreement.status, 0);

    setup
        .settlement_client
        .mark_defaulted(&setup.admin, &agreement_id);

    agreement = setup.settlement_client.get_agreement(&agreement_id).unwrap();
    assert_eq!(agreement.status, 2);
}

#[test]
#[should_panic(expected = "agreement not active")]
fn test_settle_inactive_agreement() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    setup
        .settlement_client
        .mark_completed(&setup.admin, &agreement_id);

    let period = String::from_str(&env, "2026-02");
    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);

    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let attested_revenue = 1_000_000i128;
    setup
        .settlement_client
        .settle(&agreement_id, &period, &attested_revenue);
}

#[test]
fn test_get_committed() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let principal = 10_000_000i128;
    let revenue_share_bps = 1000u32;
    let min_revenue = 100_000i128;
    let max_repayment = 500_000i128;

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &principal,
        &revenue_share_bps,
        &min_revenue,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let period = String::from_str(&env, "2026-02");

    let committed_before = setup.settlement_client.get_committed(&agreement_id, &period.clone());
    assert_eq!(committed_before, 0);

    let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    setup
        .attestation_client
        .submit_attestation(&setup.business, &period, &root, &1_700_000_000u64, &1u32, &None, &None);

    let attested_revenue = 1_000_000i128;
    let business_tokens = 1_500_000i128;
    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &business_tokens);

    setup
        .settlement_client
        .settle(&agreement_id, &period.clone(), &attested_revenue);

    let committed_after = setup.settlement_client.get_committed(&agreement_id, &period);
    assert_eq!(committed_after, 100_000i128);
}

#[test]
fn test_settle_multi_basic() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &10_000_000,
        &1000,
        &100_000,
        &500_000,
        &setup.attestation_id,
        &setup.token,
    );

    let periods = Vec::from_array(&env, [
        String::from_str(&env, "2026-01"),
        String::from_str(&env, "2026-02"),
    ]);

    for period in periods.iter() {
        let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
        setup.attestation_client.submit_attestation(&setup.business, &period, &root, &1700, &1, &None, &None);
    }

    let revenues = Vec::from_array(&env, [1_000_000, 2_000_000]);
    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &1_000_000);

    setup.settlement_client.settle_multi(&agreement_id, &periods, &revenues);

    // Total revenue = 3M. Share 10% = 300k.
    // 300k distributed as 150k per period.
    let s1 = setup.settlement_client.get_settlement(&agreement_id, &periods.get(0).unwrap()).unwrap();
    let s2 = setup.settlement_client.get_settlement(&agreement_id, &periods.get(1).unwrap()).unwrap();

    assert_eq!(s1.repayment_amount, 150_000);
    assert_eq!(s2.repayment_amount, 150_000);
}

#[test]
fn test_settle_multi_netting() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &10_000_000,
        &1000,
        &100_000,
        &500_000,
        &setup.attestation_id,
        &setup.token,
    );

    let periods = Vec::from_array(&env, [
        String::from_str(&env, "2026-01"),
        String::from_str(&env, "2026-02"),
    ]);

    for period in periods.iter() {
        let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
        setup.attestation_client.submit_attestation(&setup.business, &period, &root, &1700, &1, &None, &None);
    }

    // P1: 1M, P2: -500k. Net: 500k. 
    // Repayment: 500k * 10% = 50k.
    let revenues = Vec::from_array(&env, [1_000_000, -500_000]);
    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &1_000_000);

    setup.settlement_client.settle_multi(&agreement_id, &periods, &revenues);

    let s1 = setup.settlement_client.get_settlement(&agreement_id, &periods.get(0).unwrap()).unwrap();
    let s2 = setup.settlement_client.get_settlement(&agreement_id, &periods.get(1).unwrap()).unwrap();

    // 50k distributed: 25k each.
    assert_eq!(s1.repayment_amount, 25_000);
    assert_eq!(s2.repayment_amount, 25_000);
}

#[test]
fn test_settle_multi_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let min_threshold = 1_000_000i128;
    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &10_000_000,
        &1000,
        &min_threshold,
        &500_000,
        &setup.attestation_id,
        &setup.token,
    );

    let periods = Vec::from_array(&env, [
        String::from_str(&env, "2026-01"),
        String::from_str(&env, "2026-02"),
    ]);

    for period in periods.iter() {
        let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
        setup.attestation_client.submit_attestation(&setup.business, &period, &root, &1700, &1, &None, &None);
    }

    // Aggregated threshold = 1M * 2 = 2M.
    // P1: 1.5M, P2: 0.4M. Total: 1.9M < 2M.
    // Result: 0 repayment.
    let revenues = Vec::from_array(&env, [1_500_000, 400_000]);
    
    setup.settlement_client.settle_multi(&agreement_id, &periods, &revenues);

    let s1 = setup.settlement_client.get_settlement(&agreement_id, &periods.get(0).unwrap()).unwrap();
    assert_eq!(s1.repayment_amount, 0);
}

#[test]
fn test_settle_multi_cap() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let max_repayment = 100_000i128; // per period
    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &10_000_000,
        &2000, // 20%
        &100_000,
        &max_repayment,
        &setup.attestation_id,
        &setup.token,
    );

    let periods = Vec::from_array(&env, [
        String::from_str(&env, "2026-01"),
        String::from_str(&env, "2026-02"),
    ]);

    for period in periods.iter() {
        let root = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
        setup.attestation_client.submit_attestation(&setup.business, &period, &root, &1700, &1, &None, &None);
    }

    // Aggregated cap = 100k * 2 = 200k.
    // P1: 1M, P2: 1M. Total: 2M. 20% of 2M = 400k.
    // Capped at 200k.
    let revenues = Vec::from_array(&env, [1_000_000, 1_000_000]);
    StellarAssetClient::new(&env, &setup.token).mint(&setup.business, &1_000_000);

    setup.settlement_client.settle_multi(&agreement_id, &periods, &revenues);

    let s1 = setup.settlement_client.get_settlement(&agreement_id, &periods.get(0).unwrap()).unwrap();
    let s2 = setup.settlement_client.get_settlement(&agreement_id, &periods.get(1).unwrap()).unwrap();

    assert_eq!(s1.repayment_amount, 100_000);
    assert_eq!(s2.repayment_amount, 100_000);
}

#[test]
#[should_panic(expected = "mismatched periods and revenues")]
fn test_settle_multi_mismatched_lengths() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let setup = setup(&env);

    let agreement_id = setup.settlement_client.create_agreement(
        &setup.lender,
        &setup.business,
        &10_000_000,
        &1000,
        &100_000,
        &500_000,
        &setup.attestation_id,
        &setup.token,
    );

    let periods = Vec::from_array(&env, [String::from_str(&env, "2026-01")]);
    let revenues = Vec::from_array(&env, [1_000_000, 2_000_000]);

    setup.settlement_client.settle_multi(&agreement_id, &periods, &revenues);
}
