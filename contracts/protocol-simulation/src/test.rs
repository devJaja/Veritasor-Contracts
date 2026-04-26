#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};

struct TestContext {
    env: Env,
    contract_id: Address,
    admin: Address,
    attestation: Address,
    staking: Address,
    settlement: Address,
    lender_contract: Address,
}

fn setup() -> TestContext {
    let env = Env::default();
    let contract_id = env.register(ProtocolSimulationContract, ());
    let client = ProtocolSimulationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attestation = Address::generate(&env);
    let staking = Address::generate(&env);
    let settlement = Address::generate(&env);
    let lender_contract = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(
        &admin,
        &attestation,
        &staking,
        &settlement,
        &lender_contract,
    );

    TestContext {
        env,
        contract_id,
        admin,
        attestation,
        staking,
        settlement,
        lender_contract,
    }
}

fn client(ctx: &TestContext) -> ProtocolSimulationContractClient<'_> {
    ProtocolSimulationContractClient::new(&ctx.env, &ctx.contract_id)
}

fn seed(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn business_lifecycle_params(env: &Env, business: &Address) -> BusinessLifecycleParams {
    BusinessLifecycleParams {
        business: business.clone(),
        period: String::from_str(env, "2026-01"),
        merkle_root: seed(env, 7),
        timestamp: 1_700_000_000,
        version: 1,
        revenue_amount: 1_000_000,
    }
}

fn multi_period_params(env: &Env, business: &Address) -> MultiPeriodParams {
    let mut periods = Vec::new(env);
    periods.push_back(String::from_str(env, "2026-01"));
    periods.push_back(String::from_str(env, "2026-02"));

    let mut merkle_roots = Vec::new(env);
    merkle_roots.push_back(seed(env, 1));
    merkle_roots.push_back(seed(env, 2));

    let mut timestamps = Vec::new(env);
    timestamps.push_back(1_700_000_000u64);
    timestamps.push_back(1_700_086_400u64);

    let mut revenues = Vec::new(env);
    revenues.push_back(100_000i128);
    revenues.push_back(150_000i128);

    MultiPeriodParams {
        business: business.clone(),
        periods,
        merkle_roots,
        timestamps,
        revenues,
    }
}

#[test]
fn test_initialize_sets_contracts_and_default_seed_control() {
    let ctx = setup();
    let client = client(&ctx);

    assert_eq!(client.get_admin(), ctx.admin);
    assert_eq!(client.get_attestation_contract_address(), ctx.attestation);
    assert_eq!(client.get_staking_contract_address(), ctx.staking);
    assert_eq!(client.get_settlement_contract_address(), ctx.settlement);
    assert_eq!(client.get_lender_contract_address(), ctx.lender_contract);
    assert_eq!(client.get_scenario_count(), 0);

    let seed_control = client.get_seed_control();
    assert_eq!(seed_control.seed, seed(&ctx.env, 0));
    assert_eq!(seed_control.generation, 0);
    assert_eq!(seed_control.next_sequence, 0);
    assert_eq!(seed_control.updated_at, ctx.env.ledger().timestamp());
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_panics() {
    let ctx = setup();
    let client = client(&ctx);

    client.initialize(
        &ctx.admin,
        &ctx.attestation,
        &ctx.staking,
        &ctx.settlement,
        &ctx.lender_contract,
    );
}

#[test]
fn test_set_contracts() {
    let ctx = setup();
    let client = client(&ctx);

    let new_attestation = Address::generate(&ctx.env);
    client.set_attestation_contract(&ctx.admin, &new_attestation);
    assert_eq!(client.get_attestation_contract_address(), new_attestation);

    let new_staking = Address::generate(&ctx.env);
    client.set_staking_contract(&ctx.admin, &new_staking);
    assert_eq!(client.get_staking_contract_address(), new_staking);

    let new_settlement = Address::generate(&ctx.env);
    client.set_settlement_contract(&ctx.admin, &new_settlement);
    assert_eq!(client.get_settlement_contract_address(), new_settlement);

    let new_lender = Address::generate(&ctx.env);
    client.set_lender_contract(&ctx.admin, &new_lender);
    assert_eq!(client.get_lender_contract_address(), new_lender);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_set_contract_non_admin_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let non_admin = Address::generate(&ctx.env);
    let new_attestation = Address::generate(&ctx.env);

    client.set_attestation_contract(&non_admin, &new_attestation);
}

#[test]
fn test_set_deterministic_seed_rotates_generation_and_resets_sequence() {
    let ctx = setup();
    let client = client(&ctx);
    let seed_value = seed(&ctx.env, 9);

    client.set_deterministic_seed(&ctx.admin, &seed_value);

    let control = client.get_seed_control();
    assert_eq!(control.seed, seed_value);
    assert_eq!(control.generation, 1);
    assert_eq!(control.next_sequence, 0);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_set_deterministic_seed_non_admin_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let non_admin = Address::generate(&ctx.env);

    client.set_deterministic_seed(&non_admin, &seed(&ctx.env, 4));
}

#[test]
fn test_get_scenario_returns_none_for_nonexistent() {
    let ctx = setup();
    let client = client(&ctx);
    assert_eq!(client.get_scenario(&999), None);
    assert_eq!(client.get_scenario_seed(&999), None);
}

#[test]
fn test_preview_matches_recorded_seed_for_business_lifecycle() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let params = business_lifecycle_params(&ctx.env, &business);

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 3));

    let preview = client.preview_next_seed(
        &String::from_str(&ctx.env, "business_lifecycle"),
        &params.business,
        &params.business,
        &params.business,
        &ctx.contract_id,
    );

    let scenario_id = client.run_business_lifecycle(&params);
    let stored = client
        .get_scenario_seed(&scenario_id)
        .expect("scenario seed must be stored");

    assert_eq!(scenario_id, 0);
    assert_eq!(stored, preview);

    let scenario = client
        .get_scenario(&scenario_id)
        .expect("scenario must exist");
    assert_eq!(
        scenario.name,
        String::from_str(&ctx.env, "business_lifecycle")
    );
    assert_eq!(scenario.status, 2);

    let control = client.get_seed_control();
    assert_eq!(control.generation, 1);
    assert_eq!(control.next_sequence, 1);
}

#[test]
fn test_scenario_ordering_changes_derived_seed() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let params = business_lifecycle_params(&ctx.env, &business);

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 5));

    let first_id = client.run_business_lifecycle(&params);
    let second_id = client.run_business_lifecycle(&params);

    let first = client.get_scenario_seed(&first_id).unwrap();
    let second = client.get_scenario_seed(&second_id).unwrap();

    assert_eq!(first.sequence, 0);
    assert_eq!(second.sequence, 1);
    assert_ne!(first.derived_seed, second.derived_seed);
}

#[test]
fn test_same_seed_and_inputs_are_reproducible_across_fresh_envs() {
    let ctx_a = setup();
    let client_a = client(&ctx_a);
    let business_a = Address::generate(&ctx_a.env);
    let params_a = business_lifecycle_params(&ctx_a.env, &business_a);
    client_a.set_deterministic_seed(&ctx_a.admin, &seed(&ctx_a.env, 11));
    let scenario_id_a = client_a.run_business_lifecycle(&params_a);
    let record_a = client_a.get_scenario_seed(&scenario_id_a).unwrap();

    let ctx_b = setup();
    let client_b = client(&ctx_b);
    let business_b = Address::generate(&ctx_b.env);
    let params_b = business_lifecycle_params(&ctx_b.env, &business_b);
    client_b.set_deterministic_seed(&ctx_b.admin, &seed(&ctx_b.env, 11));
    let scenario_id_b = client_b.run_business_lifecycle(&params_b);
    let record_b = client_b.get_scenario_seed(&scenario_id_b).unwrap();

    assert_eq!(scenario_id_a, scenario_id_b);
    assert_eq!(record_a, record_b);
}

#[test]
fn test_reseeding_same_raw_seed_changes_generation_and_prevents_replay_collision() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let raw_seed = seed(&ctx.env, 12);

    client.set_deterministic_seed(&ctx.admin, &raw_seed);
    let first_preview = client.preview_next_seed(
        &String::from_str(&ctx.env, "business_lifecycle"),
        &business,
        &business,
        &business,
        &ctx.contract_id,
    );

    client.set_deterministic_seed(&ctx.admin, &raw_seed);
    let second_preview = client.preview_next_seed(
        &String::from_str(&ctx.env, "business_lifecycle"),
        &business,
        &business,
        &business,
        &ctx.contract_id,
    );

    assert_eq!(first_preview.sequence, 0);
    assert_eq!(second_preview.sequence, 0);
    assert_eq!(first_preview.scenario_id, second_preview.scenario_id);
    assert_ne!(first_preview.generation, second_preview.generation);
    assert_ne!(first_preview.derived_seed, second_preview.derived_seed);
}

#[test]
fn test_multi_period_preview_matches_recorded_seed() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let params = multi_period_params(&ctx.env, &business);

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 14));

    let preview = client.preview_next_seed(
        &String::from_str(&ctx.env, "multi_period"),
        &business,
        &business,
        &business,
        &ctx.contract_id,
    );

    let scenario_id = client.run_multi_period_scenario(&params);
    let stored = client.get_scenario_seed(&scenario_id).unwrap();
    let scenario = client.get_scenario(&scenario_id).unwrap();

    assert_eq!(stored, preview);
    assert_eq!(scenario.status, 2);
}

#[test]
#[should_panic(expected = "periods and timestamps length mismatch")]
fn test_multi_period_length_mismatch_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let mut params = multi_period_params(&ctx.env, &business);
    params.timestamps.pop_back();

    client.run_multi_period_scenario(&params);
}

#[test]
fn test_lender_integration_params_creation() {
    let ctx = setup();
    let lender = Address::generate(&ctx.env);
    let business = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let params = LenderIntegrationParams {
        lender: lender.clone(),
        business: business.clone(),
        principal: 100_000i128,
        revenue_share_bps: 500u32,
        min_revenue_threshold: 10_000i128,
        max_repayment_amount: 5_000i128,
        token: token.clone(),
    };

    assert_eq!(params.lender, lender);
    assert_eq!(params.business, business);
    assert_eq!(params.principal, 100_000);
    assert_eq!(params.revenue_share_bps, 500);
    assert_eq!(params.min_revenue_threshold, 10_000);
    assert_eq!(params.max_repayment_amount, 5_000);
    assert_eq!(params.token, token);
}

#[test]
fn test_staking_scenario_params_creation() {
    let ctx = setup();
    let attestor = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let params = StakingScenarioParams {
        attestor: attestor.clone(),
        stake_amount: 50_000i128,
        token: token.clone(),
    };

    assert_eq!(params.attestor, attestor);
    assert_eq!(params.stake_amount, 50_000);
    assert_eq!(params.token, token);
}

#[test]
fn test_multi_period_params_creation() {
    let ctx = setup();
    let business = Address::generate(&ctx.env);
    let params = multi_period_params(&ctx.env, &business);

    assert_eq!(params.business, business);
    assert_eq!(params.periods.len(), 2);
    assert_eq!(params.merkle_roots.len(), 2);
    assert_eq!(params.timestamps.len(), 2);
    assert_eq!(params.revenues.len(), 2);
}

#[test]
fn test_scenario_result_creation() {
    let result = ScenarioResult {
        scenario_id: 1,
        success: true,
        steps_completed: 3,
        error_message: None,
        completed_at: 1_700_000_000,
    };

    assert_eq!(result.scenario_id, 1);
    assert!(result.success);
    assert_eq!(result.steps_completed, 3);
    assert_eq!(result.error_message, None);
    assert_eq!(result.completed_at, 1_700_000_000);
}

#[test]
fn test_scenario_result_with_error() {
    let env = Env::default();
    let error_msg = String::from_str(&env, "test_error");

    let result = ScenarioResult {
        scenario_id: 2,
        success: false,
        steps_completed: 1,
        error_message: Some(error_msg.clone()),
        completed_at: 1_700_000_000,
    };

    assert_eq!(result.scenario_id, 2);
    assert!(!result.success);
    assert_eq!(result.steps_completed, 1);
    assert_eq!(result.error_message, Some(error_msg));
}

#[test]
fn test_revocation_scenario_params_creation() {
    let ctx = setup();
    let business = Address::generate(&ctx.env);
    let admin = Address::generate(&ctx.env);

    let params = (
        admin.clone(),
        business.clone(),
        String::from_str(&ctx.env, "2026-01"),
        seed(&ctx.env, 8),
        String::from_str(&ctx.env, "test_revocation"),
    );

    assert_eq!(params.0, admin);
    assert_eq!(params.1, business);
    assert_eq!(params.2, String::from_str(&ctx.env, "2026-01"));
    assert_eq!(params.3, seed(&ctx.env, 8));
    assert_eq!(params.4, String::from_str(&ctx.env, "test_revocation"));
}

#[test]
fn test_revocation_scenario_execution() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 15));

    let scenario_id = client.run_revocation_scenario(
        &ctx.admin,
        &business,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 9),
        &String::from_str(&ctx.env, "test_reason"),
    );

    let scenario = client.get_scenario(&scenario_id).unwrap();
    assert_eq!(scenario.status, 2); // completed
    assert_eq!(
        scenario.name,
        String::from_str(&ctx.env, "revocation_scenario")
    );

    let seed_record = client.get_scenario_seed(&scenario_id).unwrap();
    assert_eq!(seed_record.scenario_id, scenario_id);
    assert_eq!(seed_record.generation, 1);
    assert_eq!(seed_record.sequence, 0);
}

#[test]
fn test_lender_integration_scenario_execution() {
    let ctx = setup();
    let client = client(&ctx);
    let lender = Address::generate(&ctx.env);
    let business = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let params = LenderIntegrationParams {
        lender: lender.clone(),
        business: business.clone(),
        principal: 100_000i128,
        revenue_share_bps: 500u32,
        min_revenue_threshold: 10_000i128,
        max_repayment_amount: 5_000i128,
        token: token.clone(),
    };

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 16));

    let scenario_id = client.run_lender_integration(
        &params,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 10),
        1_700_000_000u64,
        50_000i128,
    );

    let scenario = client.get_scenario(&scenario_id).unwrap();
    assert_eq!(scenario.status, 2); // completed
    assert_eq!(
        scenario.name,
        String::from_str(&ctx.env, "lender_integration")
    );

    let seed_record = client.get_scenario_seed(&scenario_id).unwrap();
    assert_eq!(seed_record.scenario_id, scenario_id);
    assert_eq!(seed_record.generation, 1);
    assert_eq!(seed_record.sequence, 0);
}

#[test]
fn test_staking_scenario_execution() {
    let ctx = setup();
    let client = client(&ctx);
    let attestor = Address::generate(&ctx.env);
    let business = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let params = StakingScenarioParams {
        attestor: attestor.clone(),
        stake_amount: 50_000i128,
        token: token.clone(),
    };

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 17));

    let scenario_id = client.run_staking_scenario(
        &params,
        &business,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 11),
    );

    let scenario = client.get_scenario(&scenario_id).unwrap();
    assert_eq!(scenario.status, 2); // completed
    assert_eq!(
        scenario.name,
        String::from_str(&ctx.env, "staking_scenario")
    );

    let seed_record = client.get_scenario_seed(&scenario_id).unwrap();
    assert_eq!(seed_record.scenario_id, scenario_id);
    assert_eq!(seed_record.generation, 1);
    assert_eq!(seed_record.sequence, 0);
}

#[test]
fn test_deterministic_seed_control_structure() {
    let ctx = setup();
    let client = client(&ctx);

    let control = client.get_seed_control();
    assert_eq!(control.seed, seed(&ctx.env, 0));
    assert_eq!(control.generation, 0);
    assert_eq!(control.next_sequence, 0);
    assert!(control.updated_at > 0);
}

#[test]
fn test_scenario_seed_record_structure() {
    let env = Env::default();
    let record = ScenarioSeedRecord {
        scenario_id: 42,
        generation: 5,
        sequence: 3,
        derived_seed: seed(&env, 99),
    };

    assert_eq!(record.scenario_id, 42);
    assert_eq!(record.generation, 5);
    assert_eq!(record.sequence, 3);
    assert_eq!(record.derived_seed, seed(&env, 99));
}

#[test]
fn test_preview_next_seed_without_mutation() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 20));

    let initial_control = client.get_seed_control();
    let initial_count = client.get_scenario_count();

    let preview = client.preview_next_seed(
        &String::from_str(&ctx.env, "test_scenario"),
        &business,
        &business,
        &business,
        &ctx.contract_id,
    );

    let final_control = client.get_seed_control();
    let final_count = client.get_scenario_count();

    // Verify no state mutation occurred
    assert_eq!(initial_control.seed, final_control.seed);
    assert_eq!(initial_control.generation, final_control.generation);
    assert_eq!(initial_control.next_sequence, final_control.next_sequence);
    assert_eq!(initial_count, final_count);

    // Verify preview structure
    assert_eq!(preview.scenario_id, initial_count);
    assert_eq!(preview.generation, initial_control.generation);
    assert_eq!(preview.sequence, initial_control.next_sequence);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_revocation_scenario_non_admin_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let non_admin = Address::generate(&ctx.env);

    client.run_revocation_scenario(
        &non_admin,
        &business,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 8),
        &String::from_str(&ctx.env, "test_reason"),
    );
}

#[test]
fn test_business_lifecycle_scenario_execution() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let params = business_lifecycle_params(&ctx.env, &business);

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 18));

    let scenario_id = client.run_business_lifecycle(&params);

    let scenario = client.get_scenario(&scenario_id).unwrap();
    assert_eq!(scenario.status, 2); // completed
    assert_eq!(
        scenario.name,
        String::from_str(&ctx.env, "business_lifecycle")
    );
    assert_eq!(scenario.business, business);

    let seed_record = client.get_scenario_seed(&scenario_id).unwrap();
    assert_eq!(seed_record.scenario_id, scenario_id);
    assert_eq!(seed_record.generation, 1);
    assert_eq!(seed_record.sequence, 0);
}

#[test]
fn test_scenario_config_structure() {
    let env = Env::default();
    let business = Address::generate(&env);
    let lender = Address::generate(&env);
    let attestor = Address::generate(&env);
    let token = Address::generate(&env);

    let config = ScenarioConfig {
        id: 123,
        name: String::from_str(&env, "test_scenario"),
        business: business.clone(),
        lender: lender.clone(),
        attestor: attestor.clone(),
        token: token.clone(),
        created_at: 1_700_000_000,
        status: 1,
    };

    assert_eq!(config.id, 123);
    assert_eq!(config.name, String::from_str(&env, "test_scenario"));
    assert_eq!(config.business, business);
    assert_eq!(config.lender, lender);
    assert_eq!(config.attestor, attestor);
    assert_eq!(config.token, token);
    assert_eq!(config.created_at, 1_700_000_000);
    assert_eq!(config.status, 1);
}

#[test]
fn test_data_key_enum_coverage() {
    // Test that all DataKey variants can be constructed
    let env = Env::default();

    // This test ensures all enum variants are accessible
    // and helps with code coverage analysis
    let _admin_key = DataKey::Admin;
    let _scenario_count_key = DataKey::ScenarioCount;
    let _scenario_key = DataKey::Scenario(42);
    let _scenario_seed_key = DataKey::ScenarioSeed(42);
    let _attestation_key = DataKey::AttestationContract;
    let _staking_key = DataKey::StakingContract;
    let _settlement_key = DataKey::SettlementContract;
    let _lender_key = DataKey::LenderContract;
    let _seed_control_key = DataKey::SeedControl;

    // Verify enum variants are distinct
    assert_ne!(
        format!("{:?}", _admin_key),
        format!("{:?}", _scenario_count_key)
    );
}

#[test]
fn test_zero_seed_generation() {
    let env = Env::default();
    let zero = seed(&env, 0);

    // Verify zero seed is consistently generated
    assert_eq!(zero, seed(&env, 0));

    // Verify it's different from non-zero seeds
    assert_ne!(zero, seed(&env, 1));
    assert_ne!(zero, seed(&env, 255));
}

#[test]
fn test_multiple_scenarios_increasing_sequence() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let params = business_lifecycle_params(&ctx.env, &business);

    client.set_deterministic_seed(&ctx.admin, &seed(&ctx.env, 25));

    let scenario1 = client.run_business_lifecycle(&params);
    let scenario2 = client.run_business_lifecycle(&params);
    let scenario3 = client.run_business_lifecycle(&params);

    let seed1 = client.get_scenario_seed(&scenario1).unwrap();
    let seed2 = client.get_scenario_seed(&scenario2).unwrap();
    let seed3 = client.get_scenario_seed(&scenario3).unwrap();

    assert_eq!(seed1.sequence, 0);
    assert_eq!(seed2.sequence, 1);
    assert_eq!(seed3.sequence, 2);

    // Verify all seeds are different
    assert_ne!(seed1.derived_seed, seed2.derived_seed);
    assert_ne!(seed2.derived_seed, seed3.derived_seed);
    assert_ne!(seed1.derived_seed, seed3.derived_seed);
}

#[test]
#[should_panic(expected = "principal must be positive")]
fn test_lender_integration_negative_principal_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let lender = Address::generate(&ctx.env);
    let business = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let params = LenderIntegrationParams {
        lender: lender.clone(),
        business: business.clone(),
        principal: -1000i128, // Negative principal
        revenue_share_bps: 500u32,
        min_revenue_threshold: 10_000i128,
        max_repayment_amount: 5_000i128,
        token: token.clone(),
    };

    client.run_lender_integration(
        &params,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 10),
        1_700_000_000u64,
        50_000i128,
    );
}

#[test]
#[should_panic(expected = "revenue share basis points must be <= 10000")]
fn test_lender_integration_excessive_bps_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let lender = Address::generate(&ctx.env);
    let business = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let params = LenderIntegrationParams {
        lender: lender.clone(),
        business: business.clone(),
        principal: 100_000i128,
        revenue_share_bps: 15_000u32, // Exceeds 10000
        min_revenue_threshold: 10_000i128,
        max_repayment_amount: 5_000i128,
        token: token.clone(),
    };

    client.run_lender_integration(
        &params,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 10),
        1_700_000_000u64,
        50_000i128,
    );
}

#[test]
#[should_panic(expected = "stake amount must be positive")]
fn test_staking_scenario_zero_stake_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let attestor = Address::generate(&ctx.env);
    let business = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let params = StakingScenarioParams {
        attestor: attestor.clone(),
        stake_amount: 0i128, // Zero stake
        token: token.clone(),
    };

    client.run_staking_scenario(
        &params,
        &business,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 11),
    );
}

#[test]
#[should_panic(expected = "periods and revenues length mismatch")]
fn test_multi_period_revenues_length_mismatch_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let mut params = multi_period_params(&ctx.env, &business);
    params.revenues.pop_back(); // Remove one revenue entry

    client.run_multi_period_scenario(&params);
}

#[test]
#[should_panic(expected = "too many periods: maximum 100 allowed")]
fn test_multi_period_too_many_periods_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);

    let mut periods = Vec::new(&ctx.env);
    let mut merkle_roots = Vec::new(&ctx.env);
    let mut timestamps = Vec::new(&ctx.env);
    let mut revenues = Vec::new(&ctx.env);

    // Create 101 periods (exceeds limit)
    for i in 0..101 {
        periods.push_back(String::from_str(&ctx.env, &format!("period_{}", i)));
        merkle_roots.push_back(seed(&ctx.env, i as u8));
        timestamps.push_back(1_700_000_000u64 + (i as u64 * 86_400));
        revenues.push_back(100_000i128);
    }

    let params = MultiPeriodParams {
        business: business.clone(),
        periods,
        merkle_roots,
        timestamps,
        revenues,
    };

    client.run_multi_period_scenario(&params);
}

#[test]
#[should_panic(expected = "timestamps must be in chronological order")]
fn test_multi_period_unordered_timestamps_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);

    let mut periods = Vec::new(&ctx.env);
    let mut merkle_roots = Vec::new(&ctx.env);
    let mut timestamps = Vec::new(&ctx.env);
    let mut revenues = Vec::new(&ctx.env);

    periods.push_back(String::from_str(&ctx.env, "2026-01"));
    periods.push_back(String::from_str(&ctx.env, "2026-02"));

    merkle_roots.push_back(seed(&ctx.env, 1));
    merkle_roots.push_back(seed(&ctx.env, 2));

    // Intentionally unordered timestamps
    timestamps.push_back(1_700_086_400u64); // Later
    timestamps.push_back(1_700_000_000u64); // Earlier

    revenues.push_back(100_000i128);
    revenues.push_back(150_000i128);

    let params = MultiPeriodParams {
        business: business.clone(),
        periods,
        merkle_roots,
        timestamps,
        revenues,
    };

    client.run_multi_period_scenario(&params);
}

#[test]
#[should_panic(expected = "invalid status: must be 0-3")]
fn test_invalid_status_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let params = business_lifecycle_params(&ctx.env, &business);

    let scenario_id = client.run_business_lifecycle(&params);

    // Try to set invalid status directly through internal function
    // This tests the validation we added
    let env = &ctx.env;
    ProtocolSimulationContract::update_scenario_status(env, scenario_id, 5); // Invalid status
}

#[test]
fn test_valid_status_transitions() {
    let ctx = setup();
    let client = client(&ctx);
    let business = Address::generate(&ctx.env);
    let params = business_lifecycle_params(&ctx.env, &business);

    let scenario_id = client.run_business_lifecycle(&params);

    // Scenario should be completed (status 2) after run_business_lifecycle
    let scenario = client.get_scenario(&scenario_id).unwrap();
    assert_eq!(scenario.status, 2);
}

#[test]
#[should_panic(expected = "attested revenue must be non-negative")]
fn test_lender_integration_negative_revenue_panics() {
    let ctx = setup();
    let client = client(&ctx);
    let lender = Address::generate(&ctx.env);
    let business = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let params = LenderIntegrationParams {
        lender: lender.clone(),
        business: business.clone(),
        principal: 100_000i128,
        revenue_share_bps: 500u32,
        min_revenue_threshold: 10_000i128,
        max_repayment_amount: 5_000i128,
        token: token.clone(),
    };

    client.run_lender_integration(
        &params,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 10),
        1_700_000_000u64,
        -50_000i128, // Negative attested revenue
    );
}

#[test]
fn test_boundary_values() {
    let ctx = setup();
    let client = client(&ctx);
    let lender = Address::generate(&ctx.env);
    let business = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    // Test boundary values that should work
    let params = LenderIntegrationParams {
        lender: lender.clone(),
        business: business.clone(),
        principal: 1i128,             // Minimum positive
        revenue_share_bps: 10_000u32, // Maximum allowed
        min_revenue_threshold: 0i128, // Minimum allowed
        max_repayment_amount: 0i128,  // Minimum allowed
        token: token.clone(),
    };

    // Should not panic
    let scenario_id = client.run_lender_integration(
        &params,
        &String::from_str(&ctx.env, "2026-01"),
        &seed(&ctx.env, 10),
        1_700_000_000u64,
        0i128, // Minimum attested revenue
    );

    let scenario = client.get_scenario(&scenario_id).unwrap();
    assert_eq!(scenario.status, 2);
}
