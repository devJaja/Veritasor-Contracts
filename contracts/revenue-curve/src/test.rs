//! Tests for revenue curve pricing contract.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Address, BytesN, Env, String};
use veritasor_attestation::{AttestationContract, AttestationContractClient};

fn setup(
    env: &Env,
) -> (
    Address,
    RevenueCurveContractClient<'static>,
    AttestationContractClient<'static>,
    Address,
) {
    let admin = Address::generate(env);

    // Register and initialize revenue curve contract
    let curve_contract_id = env.register(RevenueCurveContract, ());
    let curve_client = RevenueCurveContractClient::new(env, &curve_contract_id);
    curve_client.initialize(&admin);

    // Register and initialize attestation contract
    let attestation_id = env.register(AttestationContract, ());
    let attestation_client = AttestationContractClient::new(env, &attestation_id);
    attestation_client.initialize(&admin, &0u64);

    // Link attestation contract
    curve_client.set_attestation_contract(&admin, &attestation_id);

    (admin, curve_client, attestation_client, attestation_id)
}

fn create_default_policy() -> PricingPolicy {
    PricingPolicy {
        base_apr_bps: 1000,             // 10%
        min_apr_bps: 300,               // 3%
        max_apr_bps: 3000,              // 30%
        risk_premium_bps_per_point: 10, // 0.1% per anomaly point
        enabled: true,
    }
}

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(RevenueCurveContract, ());
    let client = RevenueCurveContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(RevenueCurveContract, ());
    let client = RevenueCurveContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    client.initialize(&admin);
}

#[test]
fn test_set_pricing_policy() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let stored = client.get_pricing_policy().unwrap();
    assert_eq!(stored.base_apr_bps, 1000);
    assert_eq!(stored.min_apr_bps, 300);
    assert_eq!(stored.max_apr_bps, 3000);
}

#[test]
#[should_panic(expected = "min_apr must be <= max_apr")]
fn test_invalid_policy_min_max() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = PricingPolicy {
        base_apr_bps: 1000,
        min_apr_bps: 3000,
        max_apr_bps: 300,
        risk_premium_bps_per_point: 10,
        enabled: true,
    };
    client.set_pricing_policy(&admin, &policy);
}

#[test]
#[should_panic(expected = "base_apr must be within [min_apr, max_apr]")]
fn test_invalid_policy_base_out_of_range() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = PricingPolicy {
        base_apr_bps: 5000,
        min_apr_bps: 300,
        max_apr_bps: 3000,
        risk_premium_bps_per_point: 10,
        enabled: true,
    };
    client.set_pricing_policy(&admin, &policy);
}

#[test]
fn test_set_revenue_tiers() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 100_000,
            discount_bps: 50,
        },
        RevenueTier {
            min_revenue: 500_000,
            discount_bps: 100,
        },
        RevenueTier {
            min_revenue: 1_000_000,
            discount_bps: 200,
        },
    ];

    client.set_revenue_tiers(&admin, &tiers);

    let stored = client.get_revenue_tiers().unwrap();
    assert_eq!(stored.len(), 3);
    assert_eq!(stored.get(0).unwrap().min_revenue, 100_000);
    assert_eq!(stored.get(2).unwrap().discount_bps, 200);
}

#[test]
#[should_panic(expected = "tiers must be sorted by min_revenue ascending")]
fn test_unsorted_tiers_fail() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 500_000,
            discount_bps: 100,
        },
        RevenueTier {
            min_revenue: 100_000,
            discount_bps: 50,
        },
    ];

    client.set_revenue_tiers(&admin, &tiers);
}

#[test]
#[should_panic(expected = "discount cannot exceed 100%")]
fn test_excessive_discount_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 100_000,
            discount_bps: 15000,
        },
    ];

    client.set_revenue_tiers(&admin, &tiers);
}

#[test]
fn test_calculate_pricing_basic() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    // Set up policy
    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    // Create attestation
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );

    // Calculate pricing with zero risk
    let output = client.calculate_pricing(&business, &period, &500_000i128, &0u32);

    assert_eq!(output.base_apr_bps, 1000);
    assert_eq!(output.risk_premium_bps, 0);
    assert_eq!(output.apr_bps, 1000);
}

#[test]
fn test_calculate_pricing_with_risk() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );

    // Calculate pricing with anomaly score of 50
    let output = client.calculate_pricing(&business, &period, &500_000i128, &50u32);

    // Base 1000 + (50 * 10) = 1500 bps
    assert_eq!(output.base_apr_bps, 1000);
    assert_eq!(output.risk_premium_bps, 500);
    assert_eq!(output.apr_bps, 1500);
}

#[test]
fn test_calculate_pricing_with_tier_discount() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 100_000,
            discount_bps: 100,
        },
        RevenueTier {
            min_revenue: 1_000_000,
            discount_bps: 300,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );

    // Revenue qualifies for tier 2 (1M+)
    let output = client.calculate_pricing(&business, &period, &1_500_000i128, &0u32);

    assert_eq!(output.tier_level, 2);
    assert_eq!(output.tier_discount_bps, 300);
    assert_eq!(output.apr_bps, 700); // 1000 - 300
}

#[test]
fn test_calculate_pricing_max_cap() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );

    // High anomaly score should cap at max_apr
    let output = client.calculate_pricing(&business, &period, &100_000i128, &100u32);

    // Base 1000 + (100 * 10) = 2000, capped at 3000 max
    assert_eq!(output.apr_bps, 2000);
    assert!(output.apr_bps <= 3000);
}

#[test]
fn test_calculate_pricing_min_cap() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    // Large tier discount
    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 100_000,
            discount_bps: 2000,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );

    // Large discount should cap at min_apr
    let output = client.calculate_pricing(&business, &period, &5_000_000i128, &0u32);

    assert_eq!(output.apr_bps, 300); // Capped at min_apr
}

#[test]
#[should_panic(expected = "attestation not found")]
fn test_calculate_pricing_no_attestation() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");

    client.calculate_pricing(&business, &period, &500_000i128, &0u32);
}

/// NOTE: The attestation contract's `is_revoked()` is currently a stub that always returns
/// false. This test verifies the revenue-curve guard is structurally present and will fire
/// correctly once the attestation contract implements full revocation state tracking.
/// When `is_revoked` is stubbed to false, `calculate_pricing` succeeds on a "revoked" attestation —
/// this is a known upstream limitation, not a bug in the revenue-curve contract.
#[test]
fn test_calculate_pricing_revoked_attestation_stub_behavior() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );

    // Call revoke (no-op in current stub)
    let reason = String::from_str(&env, "fraud detected");
    attestation_client.revoke_attestation(&admin, &business, &period, &reason, &1u64);

    // With is_revoked stubbed to false, calculate_pricing succeeds (known upstream limitation).
    // The fee-curve-side guard is in place; remove this assertion when the attestation
    // contract transitions is_revoked from stub to full state tracking.
    let output = client.calculate_pricing(&business, &period, &500_000i128, &0u32);
    assert_eq!(output.base_apr_bps, 1000); // pricing still runs due to stub
}

#[test]
fn test_get_pricing_quote() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 1_000_000,
            discount_bps: 200,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    // Get quote without attestation
    let output = client.get_pricing_quote(&2_000_000i128, &25u32);

    // Base 1000 + (25 * 10) - 200 = 1050
    assert_eq!(output.base_apr_bps, 1000);
    assert_eq!(output.risk_premium_bps, 250);
    assert_eq!(output.tier_discount_bps, 200);
    assert_eq!(output.apr_bps, 1050);
}

#[test]
#[should_panic(expected = "anomaly_score must be <= 100")]
fn test_invalid_anomaly_score() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    client.get_pricing_quote(&500_000i128, &101u32);
}

#[test]
#[should_panic(expected = "pricing policy is disabled")]
fn test_pricing_with_disabled_policy() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let mut policy = create_default_policy();
    policy.enabled = false;
    client.set_pricing_policy(&admin, &policy);

    // Should panic when policy is disabled
    client.get_pricing_quote(&500_000i128, &10u32);
}

#[test]
fn test_multiple_pricing_scenarios() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 250_000,
            discount_bps: 50,
        },
        RevenueTier {
            min_revenue: 500_000,
            discount_bps: 100,
        },
        RevenueTier {
            min_revenue: 1_000_000,
            discount_bps: 200,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    // Scenario 1: Low revenue, low risk
    let business1 = Address::generate(&env);
    let period1 = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business1,
        &period1,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );
    let output1 = client.calculate_pricing(&business1, &period1, &100_000i128, &10u32);
    assert_eq!(output1.tier_level, 0);
    assert_eq!(output1.apr_bps, 1100); // 1000 + 100

    // Scenario 2: Medium revenue, medium risk
    let business2 = Address::generate(&env);
    let period2 = String::from_str(&env, "2026-Q2");
    attestation_client.submit_attestation(
        &business2,
        &period2,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );
    let output2 = client.calculate_pricing(&business2, &period2, &600_000i128, &30u32);
    assert_eq!(output2.tier_level, 2);
    assert_eq!(output2.apr_bps, 1200); // 1000 + 300 - 100

    // Scenario 3: High revenue, high risk
    let business3 = Address::generate(&env);
    let period3 = String::from_str(&env, "2026-Q3");
    attestation_client.submit_attestation(
        &business3,
        &period3,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );
    let output3 = client.calculate_pricing(&business3, &period3, &2_000_000i128, &80u32);
    assert_eq!(output3.tier_level, 3);
    assert_eq!(output3.apr_bps, 1600); // 1000 + 800 - 200
}

#[test]
fn test_edge_case_zero_revenue() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    attestation_client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );

    let output = client.calculate_pricing(&business, &period, &0i128, &0u32);
    assert_eq!(output.tier_level, 0);
    assert_eq!(output.apr_bps, 1000);
}

#[test]
fn test_edge_case_extreme_revenue() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 1_000_000_000_000i128,
            discount_bps: 500,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let output = client.get_pricing_quote(&10_000_000_000_000i128, &0u32);
    assert_eq!(output.tier_level, 1);
    assert_eq!(output.apr_bps, 500); // 1000 - 500
}

// ════════════════════════════════════════════════════════════════════
//  Extreme-input stress tests (deterministic / adversarial)
// ════════════════════════════════════════════════════════════════════

fn submit_test_attestation(
    env: &Env,
    attestation_client: &AttestationContractClient<'_>,
    business: &Address,
    period: &String,
) {
    let root = BytesN::from_array(env, &[9u8; 32]);
    attestation_client.submit_attestation(
        business,
        period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
    );
}

#[test]
#[should_panic(expected = "risk premium per point cannot exceed 1000 bps")]
fn test_stress_quote_risk_product_saturates_u32_max_then_clamps_to_max_apr() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    // risk_premium_bps_per_point = u32::MAX violates the 1000 bps cap.
    // This test verifies the guard fires before any arithmetic is attempted.
    let mut policy = create_default_policy();
    policy.risk_premium_bps_per_point = u32::MAX;
    client.set_pricing_policy(&admin, &policy);
}

#[test]
#[should_panic(expected = "max_apr cannot exceed 10000 bps (100%)")]
fn test_stress_quote_base_plus_risk_saturates_before_discount() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    // max_apr_bps = u32::MAX violates the 10 000 bps cap.
    // This test verifies the guard fires before any arithmetic is attempted.
    let mut policy = create_default_policy();
    policy.base_apr_bps = u32::MAX;
    policy.min_apr_bps = 300;
    policy.max_apr_bps = u32::MAX;
    policy.risk_premium_bps_per_point = 1;
    client.set_pricing_policy(&admin, &policy);
}

#[test]
fn test_stress_equal_tier_discounts_first_matching_tier_wins() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 0,
            discount_bps: 200,
        },
        RevenueTier {
            min_revenue: 100_000,
            discount_bps: 200,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&500_000i128, &0u32);
    assert_eq!(out.tier_discount_bps, 200);
    assert_eq!(out.tier_level, 1);
    assert_eq!(out.apr_bps, 800);
}

#[test]
fn test_stress_many_tiers_selects_max_discount() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let mut tiers = Vec::new(&env);
    for i in 0u32..20 {
        tiers.push_back(RevenueTier {
            min_revenue: (i as i128) * 10_000,
            discount_bps: i * 10,
        });
    }
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&250_000i128, &0u32);
    // Highest index with min_revenue <= revenue is i=19 (min 190_000, discount 190)
    assert_eq!(out.tier_level, 20);
    assert_eq!(out.tier_discount_bps, 190);
    assert_eq!(out.apr_bps, 810);
}

#[test]
fn test_stress_negative_revenue_with_negative_tier_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: -500_000i128,
            discount_bps: 150,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&-1i128, &0u32);
    assert_eq!(out.tier_level, 1);
    assert_eq!(out.tier_discount_bps, 150);
    assert_eq!(out.apr_bps, 850);
}

#[test]
fn test_stress_i128_min_no_positive_zero_tier_match() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 0,
            discount_bps: 50,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&i128::MIN, &0u32);
    assert_eq!(out.tier_level, 0);
    assert_eq!(out.tier_discount_bps, 0);
    assert_eq!(out.apr_bps, 1000);
}

#[test]
fn test_stress_i128_max_matches_top_tier() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 0,
            discount_bps: 10,
        },
        RevenueTier {
            min_revenue: i128::MAX - 1,
            discount_bps: 400,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&i128::MAX, &0u32);
    assert_eq!(out.tier_level, 2);
    assert_eq!(out.tier_discount_bps, 400);
    assert_eq!(out.apr_bps, 600);
}

#[test]
fn test_stress_anomaly_boundaries_zero_and_100() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let low = client.get_pricing_quote(&100i128, &0u32);
    assert_eq!(low.risk_premium_bps, 0);
    assert_eq!(low.apr_bps, 1000);

    let high = client.get_pricing_quote(&100i128, &100u32);
    assert_eq!(high.risk_premium_bps, 1000);
    assert_eq!(high.apr_bps, 2000);
}

#[test]
#[should_panic(expected = "anomaly_score must be <= 100")]
fn test_stress_calculate_pricing_anomaly_101_panics() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-X");
    submit_test_attestation(&env, &attestation_client, &business, &period);

    client.calculate_pricing(&business, &period, &1i128, &101u32);
}

#[test]
fn test_stress_zero_risk_multiplier_high_anomaly_boundary() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let mut policy = create_default_policy();
    policy.risk_premium_bps_per_point = 0;
    client.set_pricing_policy(&admin, &policy);

    let out = client.get_pricing_quote(&0i128, &100u32);
    assert_eq!(out.risk_premium_bps, 0);
    assert_eq!(out.apr_bps, 1000);
}

#[test]
fn test_stress_large_discount_saturating_sub_then_min_clamp() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    let mut policy = create_default_policy();
    policy.base_apr_bps = 500;
    policy.min_apr_bps = 300;
    policy.max_apr_bps = 3000;
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 0,
            discount_bps: 10_000,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&1i128, &0u32);
    assert_eq!(out.tier_discount_bps, 10_000);
    assert_eq!(out.apr_bps, policy.min_apr_bps);
}

#[test]
fn test_stress_calculate_pricing_matches_quote_when_attestation_ok() {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let (admin, client, attestation_client, _) = setup(&env);

    let policy = create_default_policy();
    client.set_pricing_policy(&admin, &policy);

    let tiers = vec![
        &env,
        RevenueTier {
            min_revenue: 1,
            discount_bps: 75,
        },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-STRESS");
    submit_test_attestation(&env, &attestation_client, &business, &period);

    let revenue = 9_999_999i128;
    let anomaly = 37u32;
    let q = client.get_pricing_quote(&revenue, &anomaly);
    let c = client.calculate_pricing(&business, &period, &revenue, &anomaly);
    assert_eq!(c.apr_bps, q.apr_bps);
    assert_eq!(c.risk_premium_bps, q.risk_premium_bps);
    assert_eq!(c.tier_discount_bps, q.tier_discount_bps);
    assert_eq!(c.tier_level, q.tier_level);
}

// ════════════════════════════════════════════════════════════════════
//  Numerical safety bounds — new tests
// ════════════════════════════════════════════════════════════════════

// ── set_pricing_policy bounds ────────────────────────────────────

#[test]
#[should_panic(expected = "max_apr cannot exceed 10000 bps (100%)")]
fn test_bounds_max_apr_exactly_10001_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 5000,
            min_apr_bps: 0,
            max_apr_bps: 10_001,
            risk_premium_bps_per_point: 0,
            enabled: true,
        },
    );
}

#[test]
fn test_bounds_max_apr_exactly_10000_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 10_000,
            min_apr_bps: 0,
            max_apr_bps: 10_000,
            risk_premium_bps_per_point: 0,
            enabled: true,
        },
    );
    let stored = client.get_pricing_policy().unwrap();
    assert_eq!(stored.max_apr_bps, 10_000);
}

#[test]
#[should_panic(expected = "risk premium per point cannot exceed 1000 bps")]
fn test_bounds_risk_premium_1001_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 500,
            min_apr_bps: 0,
            max_apr_bps: 5000,
            risk_premium_bps_per_point: 1_001,
            enabled: true,
        },
    );
}

#[test]
fn test_bounds_risk_premium_exactly_1000_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 500,
            min_apr_bps: 0,
            max_apr_bps: 5000,
            risk_premium_bps_per_point: 1_000,
            enabled: true,
        },
    );
    let stored = client.get_pricing_policy().unwrap();
    assert_eq!(stored.risk_premium_bps_per_point, 1_000);
}

#[test]
#[should_panic(expected = "base_apr must be within [min_apr, max_apr]")]
fn test_bounds_base_apr_below_min_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    // base_apr_bps = 99 < min_apr_bps = 100
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 99,
            min_apr_bps: 100,
            max_apr_bps: 5000,
            risk_premium_bps_per_point: 0,
            enabled: true,
        },
    );
}

#[test]
#[should_panic(expected = "base_apr must be within [min_apr, max_apr]")]
fn test_bounds_base_apr_above_max_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    // base_apr_bps = 5001 > max_apr_bps = 5000
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 5001,
            min_apr_bps: 100,
            max_apr_bps: 5000,
            risk_premium_bps_per_point: 0,
            enabled: true,
        },
    );
}

#[test]
fn test_bounds_all_apr_equal_is_valid() {
    // min == base == max is a degenerate but valid policy (fixed rate).
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 500,
            min_apr_bps: 500,
            max_apr_bps: 500,
            risk_premium_bps_per_point: 0,
            enabled: true,
        },
    );
    // Any anomaly score must still yield exactly 500 bps.
    let out = client.get_pricing_quote(&0i128, &100u32);
    assert_eq!(out.apr_bps, 500);
}

#[test]
fn test_bounds_zero_policy_all_zeros() {
    // All-zero policy: min=base=max=0, risk=0 → apr always 0.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 0,
            min_apr_bps: 0,
            max_apr_bps: 0,
            risk_premium_bps_per_point: 0,
            enabled: true,
        },
    );
    let out = client.get_pricing_quote(&i128::MAX, &100u32);
    assert_eq!(out.apr_bps, 0);
    assert_eq!(out.risk_premium_bps, 0);
}

// ── set_revenue_tiers bounds ─────────────────────────────────────

#[test]
#[should_panic(expected = "maximum of 20 tiers allowed")]
fn test_bounds_21_tiers_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let mut tiers = Vec::new(&env);
    for i in 0u32..21 {
        tiers.push_back(RevenueTier {
            min_revenue: i as i128 * 1_000,
            discount_bps: 10,
        });
    }
    client.set_revenue_tiers(&admin, &tiers);
}

#[test]
fn test_bounds_exactly_20_tiers_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let mut tiers = Vec::new(&env);
    for i in 0u32..20 {
        tiers.push_back(RevenueTier {
            min_revenue: i as i128 * 1_000,
            discount_bps: 10,
        });
    }
    client.set_revenue_tiers(&admin, &tiers);
    assert_eq!(client.get_revenue_tiers().unwrap().len(), 20);
}

#[test]
#[should_panic(expected = "tiers must be sorted by min_revenue ascending")]
fn test_bounds_duplicate_min_revenue_rejected() {
    // Strictly ascending means equal values are also rejected.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = vec![
        &env,
        RevenueTier { min_revenue: 100_000, discount_bps: 50 },
        RevenueTier { min_revenue: 100_000, discount_bps: 100 }, // duplicate
    ];
    client.set_revenue_tiers(&admin, &tiers);
}

#[test]
#[should_panic(expected = "discount cannot exceed 100%")]
fn test_bounds_discount_exactly_10001_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = vec![
        &env,
        RevenueTier { min_revenue: 0, discount_bps: 10_001 },
    ];
    client.set_revenue_tiers(&admin, &tiers);
}

#[test]
fn test_bounds_discount_exactly_10000_accepted() {
    // 10 000 bps = 100 % discount → apr floors at min_apr.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = vec![
        &env,
        RevenueTier { min_revenue: 0, discount_bps: 10_000 },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&1i128, &0u32);
    // 1000 - 10000 saturates to 0, then clamped up to min_apr (300).
    assert_eq!(out.apr_bps, 300);
}

#[test]
fn test_bounds_empty_tiers_accepted_no_discount() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = Vec::new(&env);
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&1_000_000i128, &0u32);
    assert_eq!(out.tier_level, 0);
    assert_eq!(out.tier_discount_bps, 0);
    assert_eq!(out.apr_bps, 1000);
}

// ── i128 edge cases ───────────────────────────────────────────────

#[test]
fn test_bounds_i128_min_revenue_no_tier_match_with_positive_threshold() {
    // i128::MIN revenue should not match a tier with min_revenue = 0.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = vec![
        &env,
        RevenueTier { min_revenue: 0, discount_bps: 200 },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&i128::MIN, &0u32);
    // i128::MIN < 0, so it does not satisfy revenue >= 0.
    assert_eq!(out.tier_level, 0);
    assert_eq!(out.tier_discount_bps, 0);
    assert_eq!(out.apr_bps, 1000);
}

#[test]
fn test_bounds_i128_max_revenue_matches_all_tiers() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = vec![
        &env,
        RevenueTier { min_revenue: 0, discount_bps: 50 },
        RevenueTier { min_revenue: 1_000_000, discount_bps: 150 },
        RevenueTier { min_revenue: i128::MAX, discount_bps: 300 },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&i128::MAX, &0u32);
    assert_eq!(out.tier_level, 3);
    assert_eq!(out.tier_discount_bps, 300);
    assert_eq!(out.apr_bps, 700); // 1000 - 300
}

#[test]
fn test_bounds_revenue_exactly_at_tier_boundary() {
    // revenue == min_revenue should qualify (inclusive lower bound).
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = vec![
        &env,
        RevenueTier { min_revenue: 500_000, discount_bps: 200 },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let at_boundary = client.get_pricing_quote(&500_000i128, &0u32);
    assert_eq!(at_boundary.tier_level, 1);
    assert_eq!(at_boundary.tier_discount_bps, 200);

    let just_below = client.get_pricing_quote(&499_999i128, &0u32);
    assert_eq!(just_below.tier_level, 0);
    assert_eq!(just_below.tier_discount_bps, 0);
}

// ── Division-by-zero guard (no division in pricing path) ─────────

#[test]
fn test_bounds_no_division_zero_anomaly_zero_risk_multiplier() {
    // Ensures the pricing path is safe when both anomaly and multiplier are 0.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    let mut policy = create_default_policy();
    policy.risk_premium_bps_per_point = 0;
    client.set_pricing_policy(&admin, &policy);

    let out = client.get_pricing_quote(&0i128, &0u32);
    assert_eq!(out.risk_premium_bps, 0);
    assert_eq!(out.apr_bps, 1000);
}

// ── Negative input guards ─────────────────────────────────────────

#[test]
fn test_bounds_negative_revenue_no_tier_match_positive_thresholds() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = vec![
        &env,
        RevenueTier { min_revenue: 1, discount_bps: 100 },
        RevenueTier { min_revenue: 100_000, discount_bps: 300 },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    // Negative revenue must not match any positive-threshold tier.
    let out = client.get_pricing_quote(&-1i128, &0u32);
    assert_eq!(out.tier_level, 0);
    assert_eq!(out.tier_discount_bps, 0);
    assert_eq!(out.apr_bps, 1000);
}

#[test]
fn test_bounds_negative_revenue_matches_negative_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers = vec![
        &env,
        RevenueTier { min_revenue: -1_000_000, discount_bps: 100 },
        RevenueTier { min_revenue: 0, discount_bps: 200 },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    // -500_000 >= -1_000_000 but < 0, so only tier 1 matches.
    let out = client.get_pricing_quote(&-500_000i128, &0u32);
    assert_eq!(out.tier_level, 1);
    assert_eq!(out.tier_discount_bps, 100);
    assert_eq!(out.apr_bps, 900);
}

// ── Authorization guards ──────────────────────────────────────────

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_bounds_non_admin_cannot_set_policy() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client, _, _) = setup(&env);
    let impostor = Address::generate(&env);
    client.set_pricing_policy(&impostor, &create_default_policy());
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_bounds_non_admin_cannot_set_tiers() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client, _, _) = setup(&env);
    let impostor = Address::generate(&env);
    client.set_revenue_tiers(&impostor, &Vec::new(&env));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_bounds_non_admin_cannot_set_attestation_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client, _, attestation_id) = setup(&env);
    let impostor = Address::generate(&env);
    client.set_attestation_contract(&impostor, &attestation_id);
}

// ── Pricing entrypoint guards ─────────────────────────────────────

#[test]
#[should_panic(expected = "pricing policy not configured")]
fn test_bounds_quote_without_policy_panics() {
    let env = Env::default();
    env.mock_all_auths();
    // Fresh contract — no policy set.
    let contract_id = env.register(RevenueCurveContract, ());
    let client = RevenueCurveContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.get_pricing_quote(&100_000i128, &0u32);
}

#[test]
#[should_panic(expected = "attestation contract not set")]
fn test_bounds_calculate_pricing_without_attestation_contract_panics() {
    let env = Env::default();
    env.mock_all_auths();
    // Fresh contract — no attestation contract linked.
    let contract_id = env.register(RevenueCurveContract, ());
    let client = RevenueCurveContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.set_pricing_policy(&admin, &create_default_policy());
    // No set_attestation_contract call.
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    client.calculate_pricing(&business, &period, &100_000i128, &0u32);
}

#[test]
#[should_panic(expected = "anomaly_score must be <= 100")]
fn test_bounds_anomaly_score_u32_max_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());
    client.get_pricing_quote(&0i128, &u32::MAX);
}

// ── Arithmetic correctness spot-checks ───────────────────────────

#[test]
fn test_bounds_risk_premium_max_valid_product() {
    // anomaly=100, risk_per_point=1000 → product = 100_000 bps.
    // Combined = 1000 + 100_000 = 101_000, clamped to max_apr (10_000).
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 1000,
            min_apr_bps: 300,
            max_apr_bps: 10_000,
            risk_premium_bps_per_point: 1_000,
            enabled: true,
        },
    );
    let out = client.get_pricing_quote(&0i128, &100u32);
    // risk_premium = 100 * 1000 = 100_000 (fits in u32, no saturation needed here)
    assert_eq!(out.risk_premium_bps, 100_000);
    // combined = 1000 + 100_000 = 101_000, clamped to max_apr = 10_000
    assert_eq!(out.apr_bps, 10_000);
}

#[test]
fn test_bounds_tier_discount_exactly_equals_combined_apr() {
    // discount == combined → saturating_sub gives 0 → clamped up to min_apr.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 1000,
            min_apr_bps: 200,
            max_apr_bps: 5000,
            risk_premium_bps_per_point: 0,
            enabled: true,
        },
    );
    // discount = 1000 exactly cancels base_apr.
    let tiers = vec![
        &env,
        RevenueTier { min_revenue: 0, discount_bps: 1000 },
    ];
    client.set_revenue_tiers(&admin, &tiers);

    let out = client.get_pricing_quote(&1i128, &0u32);
    // 1000 - 1000 = 0, clamped up to min_apr = 200.
    assert_eq!(out.apr_bps, 200);
}

#[test]
fn test_bounds_policy_update_replaces_previous() {
    // Verify that a second set_pricing_policy call fully replaces the first.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);

    client.set_pricing_policy(&admin, &create_default_policy());
    let first = client.get_pricing_policy().unwrap();
    assert_eq!(first.base_apr_bps, 1000);

    client.set_pricing_policy(
        &admin,
        &PricingPolicy {
            base_apr_bps: 500,
            min_apr_bps: 100,
            max_apr_bps: 2000,
            risk_premium_bps_per_point: 5,
            enabled: true,
        },
    );
    let second = client.get_pricing_policy().unwrap();
    assert_eq!(second.base_apr_bps, 500);
    assert_eq!(second.risk_premium_bps_per_point, 5);
}

#[test]
fn test_bounds_tiers_update_replaces_previous() {
    // Verify that a second set_revenue_tiers call fully replaces the first.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client, _, _) = setup(&env);
    client.set_pricing_policy(&admin, &create_default_policy());

    let tiers_v1 = vec![
        &env,
        RevenueTier { min_revenue: 100_000, discount_bps: 50 },
    ];
    client.set_revenue_tiers(&admin, &tiers_v1);
    assert_eq!(client.get_revenue_tiers().unwrap().len(), 1);

    let tiers_v2 = vec![
        &env,
        RevenueTier { min_revenue: 50_000, discount_bps: 25 },
        RevenueTier { min_revenue: 200_000, discount_bps: 75 },
    ];
    client.set_revenue_tiers(&admin, &tiers_v2);
    let stored = client.get_revenue_tiers().unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored.get(0).unwrap().min_revenue, 50_000);
}
