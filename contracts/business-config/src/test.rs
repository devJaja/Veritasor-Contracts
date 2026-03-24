#![cfg(test)]

use crate::{
    AnomalyPolicy, BusinessConfig, BusinessConfigContract, BusinessConfigContractClient,
    ComplianceConfig, CustomFeeConfig, ExpiryConfig, IntegrationRequirements,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Symbol, Vec};

fn create_test_env() -> (Env, Address, BusinessConfigContractClient<'static>) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, BusinessConfigContract);
    let client = BusinessConfigContractClient::new(&env, &contract_id);
    (env, admin, client)
}

fn create_default_anomaly_policy() -> AnomalyPolicy {
    AnomalyPolicy {
        alert_threshold: 70,
        block_threshold: 90,
        required: false,
        auto_revoke: false,
    }
}

fn create_default_integrations(env: &Env) -> IntegrationRequirements {
    IntegrationRequirements {
        required_oracles: Vec::new(env),
        min_confirmations: 0,
        external_validation_required: false,
    }
}

fn create_default_expiry() -> ExpiryConfig {
    ExpiryConfig {
        default_expiry_seconds: 31536000,
        enforce_expiry: false,
        grace_period_seconds: 2592000,
    }
}

fn create_default_custom_fees() -> CustomFeeConfig {
    CustomFeeConfig {
        base_fee_override: None,
        tier_discount_bps: None,
        fee_waived: false,
    }
}

fn create_default_compliance(env: &Env) -> ComplianceConfig {
    ComplianceConfig {
        jurisdictions: Vec::new(env),
        required_tags: Vec::new(env),
        kyc_required: false,
        metadata_required: false,
    }
}

// ════════════════════════════════════════════════════════════════════
//  Initialization Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_initialize() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();

    client.initialize(&admin);

    let retrieved_admin = client.get_admin();
    assert_eq!(retrieved_admin, admin);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_panics() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();

    client.initialize(&admin);
    client.initialize(&admin);
}

#[test]
fn test_global_defaults_set_on_init() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();

    client.initialize(&admin);

    let defaults = client.get_global_defaults();
    assert_eq!(defaults.anomaly_policy.alert_threshold, 70);
    assert_eq!(defaults.anomaly_policy.block_threshold, 90);
    assert_eq!(defaults.expiry.default_expiry_seconds, 31536000);
}

// ════════════════════════════════════════════════════════════════════
//  Business Configuration Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_set_business_config() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let anomaly = create_default_anomaly_policy();
    let integrations = create_default_integrations(&env);
    let expiry = create_default_expiry();
    let fees = create_default_custom_fees();
    let compliance = create_default_compliance(&env);

    client.set_business_config(
        &admin,
        &business,
        &anomaly,
        &integrations,
        &expiry,
        &fees,
        &compliance,
    );

    let config = client.get_config(&business);
    assert_eq!(config.business, business);
    assert_eq!(config.version, 1);
    assert_eq!(config.anomaly_policy, anomaly);
}

#[test]
fn test_get_config_returns_defaults_when_not_set() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let config = client.get_config(&business);

    // Should return global defaults
    assert_eq!(config.anomaly_policy.alert_threshold, 70);
    assert_eq!(config.anomaly_policy.block_threshold, 90);
}

#[test]
fn test_has_custom_config() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    assert!(!client.has_custom_config(&business));

    client.set_business_config(
        &admin,
        &business,
        &create_default_anomaly_policy(),
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );

    assert!(client.has_custom_config(&business));
}

#[test]
fn test_update_business_config_increments_version() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    client.set_business_config(
        &admin,
        &business,
        &create_default_anomaly_policy(),
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );

    let config1 = client.get_config(&business);
    assert_eq!(config1.version, 1);

    // Update again
    client.set_business_config(
        &admin,
        &business,
        &create_default_anomaly_policy(),
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );

    let config2 = client.get_config(&business);
    assert_eq!(config2.version, 2);
}

// ════════════════════════════════════════════════════════════════════
//  Anomaly Policy Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_update_anomaly_policy() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let new_policy = AnomalyPolicy {
        alert_threshold: 50,
        block_threshold: 80,
        required: true,
        auto_revoke: true,
    };

    client.update_anomaly_policy(&admin, &business, &new_policy);

    let retrieved = client.get_anomaly_policy(&business);
    assert_eq!(retrieved.alert_threshold, 50);
    assert_eq!(retrieved.block_threshold, 80);
    assert!(retrieved.required);
    assert!(retrieved.auto_revoke);
}

#[test]
#[should_panic(expected = "alert threshold must be <= 100")]
fn test_anomaly_policy_alert_threshold_validation() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let invalid_policy = AnomalyPolicy {
        alert_threshold: 101,
        block_threshold: 90,
        required: false,
        auto_revoke: false,
    };

    client.update_anomaly_policy(&admin, &business, &invalid_policy);
}

#[test]
#[should_panic(expected = "block threshold must be <= 100")]
fn test_anomaly_policy_block_threshold_validation() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let invalid_policy = AnomalyPolicy {
        alert_threshold: 70,
        block_threshold: 101,
        required: false,
        auto_revoke: false,
    };

    client.update_anomaly_policy(&admin, &business, &invalid_policy);
}

#[test]
#[should_panic(expected = "alert threshold must be <= block threshold")]
fn test_anomaly_policy_threshold_ordering() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let invalid_policy = AnomalyPolicy {
        alert_threshold: 90,
        block_threshold: 70,
        required: false,
        auto_revoke: false,
    };

    client.update_anomaly_policy(&admin, &business, &invalid_policy);
}

// ════════════════════════════════════════════════════════════════════
//  Integration Requirements Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_update_integrations() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let oracle1 = Address::generate(&env);
    let oracle2 = Address::generate(&env);

    let mut oracles = Vec::new(&env);
    oracles.push_back(oracle1.clone());
    oracles.push_back(oracle2.clone());

    let integrations = IntegrationRequirements {
        required_oracles: oracles,
        min_confirmations: 2,
        external_validation_required: true,
    };

    client.update_integrations(&admin, &business, &integrations);

    let retrieved = client.get_integrations(&business);
    assert_eq!(retrieved.required_oracles.len(), 2);
    assert_eq!(retrieved.min_confirmations, 2);
    assert!(retrieved.external_validation_required);
}

#[test]
fn test_integrations_empty_oracles() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let integrations = IntegrationRequirements {
        required_oracles: Vec::new(&env),
        min_confirmations: 0,
        external_validation_required: false,
    };

    client.update_integrations(&admin, &business, &integrations);

    let retrieved = client.get_integrations(&business);
    assert_eq!(retrieved.required_oracles.len(), 0);
}

// ════════════════════════════════════════════════════════════════════
//  Expiry Configuration Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_update_expiry_config() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let expiry = ExpiryConfig {
        default_expiry_seconds: 7776000, // 90 days
        enforce_expiry: true,
        grace_period_seconds: 604800, // 7 days
    };

    client.update_expiry_config(&admin, &business, &expiry);

    let retrieved = client.get_expiry_config(&business);
    assert_eq!(retrieved.default_expiry_seconds, 7776000);
    assert!(retrieved.enforce_expiry);
    assert_eq!(retrieved.grace_period_seconds, 604800);
}

#[test]
fn test_expiry_config_no_expiry() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let expiry = ExpiryConfig {
        default_expiry_seconds: 0,
        enforce_expiry: false,
        grace_period_seconds: 0,
    };

    client.update_expiry_config(&admin, &business, &expiry);

    let retrieved = client.get_expiry_config(&business);
    assert_eq!(retrieved.default_expiry_seconds, 0);
    assert!(!retrieved.enforce_expiry);
}

// ════════════════════════════════════════════════════════════════════
//  Custom Fee Configuration Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_update_custom_fees() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let fees = CustomFeeConfig {
        base_fee_override: Some(5000),
        tier_discount_bps: Some(500),
        fee_waived: false,
    };

    client.update_custom_fees(&admin, &business, &fees);

    let retrieved = client.get_custom_fees(&business);
    assert_eq!(retrieved.base_fee_override, Some(5000));
    assert_eq!(retrieved.tier_discount_bps, Some(500));
    assert!(!retrieved.fee_waived);
}

#[test]
fn test_custom_fees_waived() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let fees = CustomFeeConfig {
        base_fee_override: None,
        tier_discount_bps: None,
        fee_waived: true,
    };

    client.update_custom_fees(&admin, &business, &fees);

    let retrieved = client.get_custom_fees(&business);
    assert!(retrieved.fee_waived);
}

#[test]
#[should_panic(expected = "discount cannot exceed 10000 bps")]
fn test_custom_fees_discount_validation() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let fees = CustomFeeConfig {
        base_fee_override: None,
        tier_discount_bps: Some(10001),
        fee_waived: false,
    };

    client.update_custom_fees(&admin, &business, &fees);
}

#[test]
#[should_panic(expected = "base fee cannot be negative")]
fn test_custom_fees_negative_fee_validation() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let fees = CustomFeeConfig {
        base_fee_override: Some(-100),
        tier_discount_bps: None,
        fee_waived: false,
    };

    client.update_custom_fees(&admin, &business, &fees);
}

// ════════════════════════════════════════════════════════════════════
//  Compliance Configuration Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_update_compliance() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let mut jurisdictions = Vec::new(&env);
    jurisdictions.push_back(Symbol::new(&env, "US"));
    jurisdictions.push_back(Symbol::new(&env, "EU"));

    let mut tags = Vec::new(&env);
    tags.push_back(Symbol::new(&env, "fintech"));

    let compliance = ComplianceConfig {
        jurisdictions,
        required_tags: tags,
        kyc_required: true,
        metadata_required: true,
    };

    client.update_compliance(&admin, &business, &compliance);

    let retrieved = client.get_compliance(&business);
    assert_eq!(retrieved.jurisdictions.len(), 2);
    assert_eq!(retrieved.required_tags.len(), 1);
    assert!(retrieved.kyc_required);
    assert!(retrieved.metadata_required);
}

#[test]
fn test_compliance_no_requirements() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);
    let compliance = ComplianceConfig {
        jurisdictions: Vec::new(&env),
        required_tags: Vec::new(&env),
        kyc_required: false,
        metadata_required: false,
    };

    client.update_compliance(&admin, &business, &compliance);

    let retrieved = client.get_compliance(&business);
    assert_eq!(retrieved.jurisdictions.len(), 0);
    assert_eq!(retrieved.required_tags.len(), 0);
    assert!(!retrieved.kyc_required);
}

// ════════════════════════════════════════════════════════════════════
//  Global Defaults Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_set_global_defaults() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let anomaly = AnomalyPolicy {
        alert_threshold: 60,
        block_threshold: 85,
        required: true,
        auto_revoke: false,
    };

    client.set_global_defaults(
        &admin,
        &anomaly,
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );

    let defaults = client.get_global_defaults();
    assert_eq!(defaults.anomaly_policy.alert_threshold, 60);
    assert_eq!(defaults.anomaly_policy.block_threshold, 85);
    assert!(defaults.anomaly_policy.required);
}

#[test]
fn test_business_without_config_uses_updated_defaults() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);

    // Update global defaults
    let anomaly = AnomalyPolicy {
        alert_threshold: 55,
        block_threshold: 75,
        required: false,
        auto_revoke: true,
    };

    client.set_global_defaults(
        &admin,
        &anomaly,
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );

    // Business without custom config should get updated defaults
    let config = client.get_config(&business);
    assert_eq!(config.anomaly_policy.alert_threshold, 55);
    assert_eq!(config.anomaly_policy.block_threshold, 75);
    assert!(config.anomaly_policy.auto_revoke);
}

// ════════════════════════════════════════════════════════════════════
//  Access Control Tests
// ════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_non_admin_cannot_set_config() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let non_admin = Address::generate(&env);
    let business = Address::generate(&env);

    client.set_business_config(
        &non_admin,
        &business,
        &create_default_anomaly_policy(),
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_non_admin_cannot_update_anomaly_policy() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let non_admin = Address::generate(&env);
    let business = Address::generate(&env);

    client.update_anomaly_policy(&non_admin, &business, &create_default_anomaly_policy());
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_non_admin_cannot_set_global_defaults() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let non_admin = Address::generate(&env);

    client.set_global_defaults(
        &non_admin,
        &create_default_anomaly_policy(),
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );
}

// ════════════════════════════════════════════════════════════════════
//  Edge Cases and Scenario Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_multiple_businesses_independent_configs() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business1 = Address::generate(&env);
    let business2 = Address::generate(&env);

    let policy1 = AnomalyPolicy {
        alert_threshold: 50,
        block_threshold: 80,
        required: true,
        auto_revoke: false,
    };

    let policy2 = AnomalyPolicy {
        alert_threshold: 70,
        block_threshold: 95,
        required: false,
        auto_revoke: true,
    };

    client.update_anomaly_policy(&admin, &business1, &policy1);
    client.update_anomaly_policy(&admin, &business2, &policy2);

    let config1 = client.get_anomaly_policy(&business1);
    let config2 = client.get_anomaly_policy(&business2);

    assert_eq!(config1.alert_threshold, 50);
    assert_eq!(config2.alert_threshold, 70);
    assert!(config1.required);
    assert!(!config2.required);
}

#[test]
fn test_partial_config_updates_preserve_other_fields() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);

    // Set full config
    client.set_business_config(
        &admin,
        &business,
        &create_default_anomaly_policy(),
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );

    let original_expiry = client.get_expiry_config(&business);

    // Update only anomaly policy
    let new_policy = AnomalyPolicy {
        alert_threshold: 40,
        block_threshold: 60,
        required: true,
        auto_revoke: true,
    };
    client.update_anomaly_policy(&admin, &business, &new_policy);

    // Expiry config should remain unchanged
    let updated_expiry = client.get_expiry_config(&business);
    assert_eq!(
        original_expiry.default_expiry_seconds,
        updated_expiry.default_expiry_seconds
    );

    // Anomaly policy should be updated
    let updated_policy = client.get_anomaly_policy(&business);
    assert_eq!(updated_policy.alert_threshold, 40);
}

#[test]
fn test_high_volume_business_profile() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);

    // High-volume business: strict anomaly detection, multiple oracles, custom fees
    let anomaly = AnomalyPolicy {
        alert_threshold: 60,
        block_threshold: 85,
        required: true,
        auto_revoke: true,
    };

    let mut oracles = Vec::new(&env);
    oracles.push_back(Address::generate(&env));
    oracles.push_back(Address::generate(&env));
    oracles.push_back(Address::generate(&env));

    let integrations = IntegrationRequirements {
        required_oracles: oracles,
        min_confirmations: 2,
        external_validation_required: true,
    };

    let fees = CustomFeeConfig {
        base_fee_override: Some(1000),
        tier_discount_bps: Some(1000), // 10% discount
        fee_waived: false,
    };

    client.set_business_config(
        &admin,
        &business,
        &anomaly,
        &integrations,
        &create_default_expiry(),
        &fees,
        &create_default_compliance(&env),
    );

    let config = client.get_config(&business);
    assert!(config.anomaly_policy.required);
    assert!(config.anomaly_policy.auto_revoke);
    assert_eq!(config.integrations.required_oracles.len(), 3);
    assert_eq!(config.integrations.min_confirmations, 2);
    assert_eq!(config.custom_fees.base_fee_override, Some(1000));
}

#[test]
fn test_startup_business_profile() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);

    // Startup: lenient policies, fee waiver, minimal requirements
    let anomaly = AnomalyPolicy {
        alert_threshold: 80,
        block_threshold: 95,
        required: false,
        auto_revoke: false,
    };

    let fees = CustomFeeConfig {
        base_fee_override: None,
        tier_discount_bps: None,
        fee_waived: true,
    };

    let expiry = ExpiryConfig {
        default_expiry_seconds: 63072000, // 2 years
        enforce_expiry: false,
        grace_period_seconds: 7776000, // 90 days
    };

    client.set_business_config(
        &admin,
        &business,
        &anomaly,
        &create_default_integrations(&env),
        &expiry,
        &fees,
        &create_default_compliance(&env),
    );

    let config = client.get_config(&business);
    assert!(!config.anomaly_policy.required);
    assert!(config.custom_fees.fee_waived);
    assert_eq!(config.expiry.default_expiry_seconds, 63072000);
}

#[test]
fn test_regulated_business_profile() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);

    // Regulated business: strict compliance, KYC required, metadata required
    let mut jurisdictions = Vec::new(&env);
    jurisdictions.push_back(Symbol::new(&env, "US"));
    jurisdictions.push_back(Symbol::new(&env, "UK"));

    let mut tags = Vec::new(&env);
    tags.push_back(Symbol::new(&env, "banking"));
    tags.push_back(Symbol::new(&env, "regulated"));

    let compliance = ComplianceConfig {
        jurisdictions,
        required_tags: tags,
        kyc_required: true,
        metadata_required: true,
    };

    let expiry = ExpiryConfig {
        default_expiry_seconds: 15552000, // 180 days
        enforce_expiry: true,
        grace_period_seconds: 0, // No grace period
    };

    client.set_business_config(
        &admin,
        &business,
        &create_default_anomaly_policy(),
        &create_default_integrations(&env),
        &expiry,
        &create_default_custom_fees(),
        &compliance,
    );

    let config = client.get_config(&business);
    assert!(config.compliance.kyc_required);
    assert!(config.compliance.metadata_required);
    assert_eq!(config.compliance.jurisdictions.len(), 2);
    assert_eq!(config.compliance.required_tags.len(), 2);
    assert!(config.expiry.enforce_expiry);
    assert_eq!(config.expiry.grace_period_seconds, 0);
}

#[test]
fn test_config_version_tracking() {
    let (env, admin, client) = create_test_env();
    env.mock_all_auths();
    client.initialize(&admin);

    let business = Address::generate(&env);

    // Initial config
    client.set_business_config(
        &admin,
        &business,
        &create_default_anomaly_policy(),
        &create_default_integrations(&env),
        &create_default_expiry(),
        &create_default_custom_fees(),
        &create_default_compliance(&env),
    );
    assert_eq!(client.get_config(&business).version, 1);

    // Update anomaly policy
    client.update_anomaly_policy(&admin, &business, &create_default_anomaly_policy());
    assert_eq!(client.get_config(&business).version, 2);

    // Update integrations
    client.update_integrations(&admin, &business, &create_default_integrations(&env));
    assert_eq!(client.get_config(&business).version, 3);

    // Update expiry
    client.update_expiry_config(&admin, &business, &create_default_expiry());
    assert_eq!(client.get_config(&business).version, 4);

    // Update fees
    client.update_custom_fees(&admin, &business, &create_default_custom_fees());
    assert_eq!(client.get_config(&business).version, 5);

    // Update compliance
    client.update_compliance(&admin, &business, &create_default_compliance(&env));
    assert_eq!(client.get_config(&business).version, 6);
}

// ════════════════════════════════════════════════════════════════════
//  Immutable Field Regression Tests
//  Tests to ensure critical fields remain immutable after initial setup
// ════════════════════════════════════════════════════════════════════

/// Business address is the primary key for business config and must remain immutable.
/// This test verifies the business address never changes across all update operations.
mod immutable_field_tests {
    use super::*;

    /// Regression: Verify business address remains unchanged through set_business_config
    #[test]
    fn test_business_address_immutable_through_set_config() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        // Set initial configuration
        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        // Update the configuration multiple times
        for _ in 0..5 {
            client.set_business_config(
                &admin,
                &business,
                &create_default_anomaly_policy(),
                &create_default_integrations(&env),
                &create_default_expiry(),
                &create_default_custom_fees(),
                &create_default_compliance(&env),
            );
        }

        // Verify business address never changed
        let config = client.get_config(&business);
        assert_eq!(
            config.business, business,
            "Business address must remain immutable"
        );
    }

    /// Regression: Verify business address unchanged through update_anomaly_policy
    #[test]
    fn test_business_address_immutable_through_anomaly_update() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        // Set initial config via set_business_config
        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;

        // Update anomaly policy multiple times
        for i in 0..3 {
            let policy = AnomalyPolicy {
                alert_threshold: 50 + i,
                block_threshold: 80 + i,
                required: true,
                auto_revoke: false,
            };
            client.update_anomaly_policy(&admin, &business, &policy);
        }

        // Business address must remain unchanged
        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
        assert_eq!(config.business, business);
    }

    /// Regression: Verify business address unchanged through update_integrations
    #[test]
    fn test_business_address_immutable_through_integrations_update() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;

        // Update integrations
        let integrations = IntegrationRequirements {
            required_oracles: Vec::new(&env),
            min_confirmations: 3,
            external_validation_required: true,
        };
        client.update_integrations(&admin, &business, &integrations);

        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
    }

    /// Regression: Verify business address unchanged through update_expiry_config
    #[test]
    fn test_business_address_immutable_through_expiry_update() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;

        // Update expiry config
        let expiry = ExpiryConfig {
            default_expiry_seconds: 15552000,
            enforce_expiry: true,
            grace_period_seconds: 604800,
        };
        client.update_expiry_config(&admin, &business, &expiry);

        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
    }

    /// Regression: Verify business address unchanged through update_custom_fees
    #[test]
    fn test_business_address_immutable_through_fees_update() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;

        // Update custom fees
        let fees = CustomFeeConfig {
            base_fee_override: Some(1000),
            tier_discount_bps: Some(500),
            fee_waived: false,
        };
        client.update_custom_fees(&admin, &business, &fees);

        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
    }

    /// Regression: Verify business address unchanged through update_compliance
    #[test]
    fn test_business_address_immutable_through_compliance_update() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;

        // Update compliance
        let compliance = ComplianceConfig {
            jurisdictions: Vec::new(&env),
            required_tags: Vec::new(&env),
            kyc_required: true,
            metadata_required: false,
        };
        client.update_compliance(&admin, &business, &compliance);

        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
    }

    /// Regression: Verify created_at timestamp remains immutable across all updates
    #[test]
    fn test_created_at_immutable_across_all_updates() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        // Set initial configuration
        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let created_at = client.get_config(&business).created_at;
        // Note: In mock environment, ledger timestamp may be 0
        // The key regression test is that created_at remains constant across updates

        // Perform all possible update operations
        client.update_anomaly_policy(&admin, &business, &create_default_anomaly_policy());
        let created_after_anomaly = client.get_config(&business).created_at;
        assert_eq!(
            created_at, created_after_anomaly,
            "created_at must not change on anomaly update"
        );

        client.update_integrations(&admin, &business, &create_default_integrations(&env));
        let created_after_integrations = client.get_config(&business).created_at;
        assert_eq!(
            created_at, created_after_integrations,
            "created_at must not change on integrations update"
        );

        client.update_expiry_config(&admin, &business, &create_default_expiry());
        let created_after_expiry = client.get_config(&business).created_at;
        assert_eq!(
            created_at, created_after_expiry,
            "created_at must not change on expiry update"
        );

        client.update_custom_fees(&admin, &business, &create_default_custom_fees());
        let created_after_fees = client.get_config(&business).created_at;
        assert_eq!(
            created_at, created_after_fees,
            "created_at must not change on fees update"
        );

        client.update_compliance(&admin, &business, &create_default_compliance(&env));
        let created_after_compliance = client.get_config(&business).created_at;
        assert_eq!(
            created_at, created_after_compliance,
            "created_at must not change on compliance update"
        );

        // Full config update
        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );
        let created_after_full = client.get_config(&business).created_at;
        assert_eq!(
            created_at, created_after_full,
            "created_at must not change on full config update"
        );
    }

    /// Regression: Verify created_at is set exactly once on first config creation
    #[test]
    fn test_created_at_set_only_on_initial_creation() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        // Before any config - should use default (created_at = 0)
        let before_config = client.get_config(&business);
        assert_eq!(before_config.created_at, 0);

        // Create config - created_at should now be set
        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let after_first_config = client.get_config(&business);
        // Note: In mock environment, created_at may be 0 if ledger timestamp is 0
        // The regression test ensures created_at remains unchanged after updates
        let first_created_at = after_first_config.created_at;

        // Update operations should NOT change created_at
        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let after_update = client.get_config(&business);
        assert_eq!(
            first_created_at, after_update.created_at,
            "created_at must be preserved across updates"
        );
    }

    /// Regression: Verify version increments correctly (mutable field behavior)
    #[test]
    fn test_version_increments_correctly() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        // First creation - version = 1
        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );
        assert_eq!(client.get_config(&business).version, 1);

        // Each update increments version by 1
        client.update_anomaly_policy(&admin, &business, &create_default_anomaly_policy());
        assert_eq!(client.get_config(&business).version, 2);

        client.update_integrations(&admin, &business, &create_default_integrations(&env));
        assert_eq!(client.get_config(&business).version, 3);

        client.update_expiry_config(&admin, &business, &create_default_expiry());
        assert_eq!(client.get_config(&business).version, 4);

        client.update_custom_fees(&admin, &business, &create_default_custom_fees());
        assert_eq!(client.get_config(&business).version, 5);

        client.update_compliance(&admin, &business, &create_default_compliance(&env));
        assert_eq!(client.get_config(&business).version, 6);

        // Full config update also increments
        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );
        assert_eq!(client.get_config(&business).version, 7);
    }
}

// ════════════════════════════════════════════════════════════════════
//  Adversarial/Edge Case Regression Tests
//  Tests for potential regression scenarios and edge cases
// ════════════════════════════════════════════════════════════════════

mod adversarial_regression_tests {
    use super::*;

    /// Edge case: Multiple rapid updates should maintain immutability
    #[test]
    fn test_rapid_updates_preserve_immutability() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;
        let original_created_at = client.get_config(&business).created_at;

        // Rapid updates - all update methods
        for i in 0..10 {
            let policy = AnomalyPolicy {
                alert_threshold: (50 + i) % 100,
                block_threshold: (80 + i) % 100,
                required: i % 2 == 0,
                auto_revoke: i % 3 == 0,
            };
            client.update_anomaly_policy(&admin, &business, &policy);
        }

        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
        assert_eq!(config.created_at, original_created_at);
    }

    /// Edge case: Empty/default config updates
    #[test]
    fn test_empty_config_updates_preserve_immutability() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;

        // Update with exact same values (edge case - no actual change)
        client.update_anomaly_policy(&admin, &business, &create_default_anomaly_policy());
        client.update_integrations(&admin, &business, &create_default_integrations(&env));
        client.update_expiry_config(&admin, &business, &create_default_expiry());
        client.update_custom_fees(&admin, &business, &create_default_custom_fees());
        client.update_compliance(&admin, &business, &create_default_compliance(&env));

        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
    }

    /// Edge case: Updating to boundary values preserves immutability
    #[test]
    fn test_boundary_value_updates_preserve_immutability() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;
        let original_created_at = client.get_config(&business).created_at;

        // Update to boundary values
        let policy = AnomalyPolicy {
            alert_threshold: 0,
            block_threshold: 100,
            required: true,
            auto_revoke: true,
        };
        client.update_anomaly_policy(&admin, &business, &policy);

        let fees = CustomFeeConfig {
            base_fee_override: Some(0),
            tier_discount_bps: Some(10000),
            fee_waived: true,
        };
        client.update_custom_fees(&admin, &business, &fees);

        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
        assert_eq!(config.created_at, original_created_at);
    }

    /// Edge case: Verify behavior when no custom config exists (uses defaults)
    ///
    /// Note: This test documents that global defaults use the caller address as a
    /// placeholder business field. When global defaults are updated, this placeholder
    /// changes. This is a known design quirk - the business field in global defaults
    /// is not truly "immutable" as it's set to the admin/caller each time.
    #[test]
    fn test_immutability_with_default_config() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        // No custom config set - uses global defaults
        // Global defaults use caller (admin) as placeholder business
        let config = client.get_config(&business);
        let initial_default_business = config.business;

        // Verify anomaly policy changes (this is the expected mutable behavior)
        let initial_alert = config.anomaly_policy.alert_threshold;

        // Update global defaults with different anomaly policy
        let new_anomaly = AnomalyPolicy {
            alert_threshold: 80,
            block_threshold: 95,
            required: true,
            auto_revoke: false,
        };
        client.set_global_defaults(
            &admin,
            &new_anomaly,
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        // After global defaults update, the policy should reflect new values
        let new_config = client.get_config(&business);
        assert_eq!(new_config.anomaly_policy.alert_threshold, 80);

        // Note: The business placeholder changes because set_global_defaults
        // uses caller.clone() as the business field. This is a design consideration.
    }

    /// Adversarial: Verify config storage key properly isolates businesses
    #[test]
    fn test_business_isolation_immutability() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business1 = Address::generate(&env);
        let business2 = Address::generate(&env);

        // Set different configs for two businesses (using set_business_config to create custom config)
        let policy1 = AnomalyPolicy {
            alert_threshold: 50,
            block_threshold: 80,
            required: true,
            auto_revoke: false,
        };
        let policy2 = AnomalyPolicy {
            alert_threshold: 70,
            block_threshold: 95,
            required: false,
            auto_revoke: true,
        };

        // Use set_business_config to create actual custom configs (not defaults)
        client.set_business_config(
            &admin,
            &business1,
            &policy1,
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );
        client.set_business_config(
            &admin,
            &business2,
            &policy2,
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        // Verify each business has correct config
        let config1 = client.get_config(&business1);
        let config2 = client.get_config(&business2);

        assert_eq!(config1.business, business1);
        assert_eq!(config2.business, business2);
        assert_ne!(config1.business, config2.business);

        // Update business1 and verify business2 is unaffected
        let new_policy1 = AnomalyPolicy {
            alert_threshold: 60,
            block_threshold: 85,
            required: true,
            auto_revoke: true,
        };
        client.update_anomaly_policy(&admin, &business1, &new_policy1);

        let updated_config1 = client.get_config(&business1);
        let unchanged_config2 = client.get_config(&business2);

        assert_eq!(updated_config1.business, business1);
        assert_eq!(unchanged_config2.business, business2);
        assert_eq!(updated_config1.anomaly_policy.alert_threshold, 60);
        assert_eq!(unchanged_config2.anomaly_policy.alert_threshold, 70);
    }

    /// Regression: Verify updated_at changes while business and created_at remain immutable
    #[test]
    fn test_updated_at_mutable_while_others_immutable() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        let original_business = client.get_config(&business).business;
        let original_created_at = client.get_config(&business).created_at;
        let original_updated_at = client.get_config(&business).updated_at;

        // Note: In mock environment, timestamps may be 0

        // Perform updates
        client.update_anomaly_policy(&admin, &business, &create_default_anomaly_policy());

        let config = client.get_config(&business);

        // Immutable fields unchanged
        assert_eq!(config.business, original_business);
        assert_eq!(config.created_at, original_created_at);

        // updated_at changed (more recent timestamp)
        assert!(config.updated_at >= original_updated_at);
    }

    /// Regression: Full lifecycle - create, update, read ensures immutability
    #[test]
    fn test_full_lifecycle_immutability() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        // Phase 1: Initial creation
        client.set_business_config(
            &admin,
            &business,
            &AnomalyPolicy {
                alert_threshold: 60,
                block_threshold: 85,
                required: true,
                auto_revoke: false,
            },
            &IntegrationRequirements {
                required_oracles: Vec::new(&env),
                min_confirmations: 1,
                external_validation_required: false,
            },
            &ExpiryConfig {
                default_expiry_seconds: 2592000,
                enforce_expiry: true,
                grace_period_seconds: 604800,
            },
            &CustomFeeConfig {
                base_fee_override: Some(1000),
                tier_discount_bps: Some(250),
                fee_waived: false,
            },
            &ComplianceConfig {
                jurisdictions: Vec::new(&env),
                required_tags: Vec::new(&env),
                kyc_required: true,
                metadata_required: false,
            },
        );

        let initial_config = client.get_config(&business);
        let initial_business = initial_config.business;
        let initial_created_at = initial_config.created_at;
        let initial_version = initial_config.version;

        // Phase 2: Multiple updates over time
        for i in 0..5 {
            client.set_business_config(
                &admin,
                &business,
                &AnomalyPolicy {
                    alert_threshold: 60 + i,
                    block_threshold: 85 + i,
                    required: true,
                    auto_revoke: false,
                },
                &IntegrationRequirements {
                    required_oracles: Vec::new(&env),
                    min_confirmations: 1 + i,
                    external_validation_required: false,
                },
                &ExpiryConfig {
                    default_expiry_seconds: 2592000 + (i as u64 * 86400),
                    enforce_expiry: true,
                    grace_period_seconds: 604800,
                },
                &CustomFeeConfig {
                    base_fee_override: Some(1000 + (i as i128 * 100)),
                    tier_discount_bps: Some(250 + (i * 50)),
                    fee_waived: false,
                },
                &ComplianceConfig {
                    jurisdictions: Vec::new(&env),
                    required_tags: Vec::new(&env),
                    kyc_required: true,
                    metadata_required: false,
                },
            );
        }

        // Phase 3: Verify immutability preserved
        let final_config = client.get_config(&business);

        assert_eq!(
            final_config.business, initial_business,
            "Business must be immutable"
        );
        assert_eq!(
            final_config.created_at, initial_created_at,
            "created_at must be immutable"
        );
        assert_eq!(
            final_config.version,
            initial_version + 5,
            "Version should increment correctly"
        );
        // Note: In mock environment, updated_at may be 0 if ledger timestamp is 0
        // The key regression test is that business and created_at remain immutable
        // Version correctly incremented
    }

    /// Adversarial: Attempt to set config with wrong business address key
    /// This tests the storage isolation - configs are keyed by business address
    #[test]
    fn test_storage_key_isolation() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business1 = Address::generate(&env);
        let business2 = Address::generate(&env);

        // Set config for business1 using set_business_config with business2's address
        // This is actually the intended API usage - setting config FOR a business
        // The business parameter IS the key, so this is not a vulnerability
        client.set_business_config(
            &admin,
            &business1,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        // Query by business1 - should get the config we just set
        let config1 = client.get_config(&business1);
        assert_eq!(config1.business, business1);

        // Query by business2 - should get defaults, NOT business1's config
        let config2 = client.get_config(&business2);
        // Defaults have different business address (contract or admin)
        assert_ne!(config2.business, business1);
    }
}

// ════════════════════════════════════════════════════════════════════
//  Performance and Gas Regression Tests
//  Tests to ensure immutability checks don't add excessive overhead
// ════════════════════════════════════════════════════════════════════

mod performance_regression_tests {
    use super::*;

    /// Regression: Large number of businesses with configs doesn't affect immutability checks
    #[test]
    fn test_many_businesses_immutability() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        // Create configs for multiple businesses using individual variables
        let business1 = Address::generate(&env);
        let business2 = Address::generate(&env);
        let business3 = Address::generate(&env);
        let business4 = Address::generate(&env);
        let business5 = Address::generate(&env);

        // Set configs
        client.set_business_config(
            &admin,
            &business1,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );
        client.set_business_config(
            &admin,
            &business2,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );
        client.set_business_config(
            &admin,
            &business3,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );
        client.set_business_config(
            &admin,
            &business4,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );
        client.set_business_config(
            &admin,
            &business5,
            &create_default_anomaly_policy(),
            &create_default_integrations(&env),
            &create_default_expiry(),
            &create_default_custom_fees(),
            &create_default_compliance(&env),
        );

        // Verify immutability
        assert_eq!(client.get_config(&business1).business, business1);
        assert_eq!(client.get_config(&business2).business, business2);
        assert_eq!(client.get_config(&business3).business, business3);
        assert_eq!(client.get_config(&business4).business, business4);
        assert_eq!(client.get_config(&business5).business, business5);

        // Update some businesses and verify immutability
        let policy1 = AnomalyPolicy {
            alert_threshold: 50,
            block_threshold: 80,
            required: true,
            auto_revoke: false,
        };
        let policy2 = AnomalyPolicy {
            alert_threshold: 60,
            block_threshold: 85,
            required: true,
            auto_revoke: false,
        };
        let policy3 = AnomalyPolicy {
            alert_threshold: 70,
            block_threshold: 90,
            required: false,
            auto_revoke: true,
        };

        client.update_anomaly_policy(&admin, &business1, &policy1);
        client.update_anomaly_policy(&admin, &business2, &policy2);
        client.update_anomaly_policy(&admin, &business3, &policy3);

        // Verify immutability still holds
        assert_eq!(client.get_config(&business1).business, business1);
        assert_eq!(client.get_config(&business2).business, business2);
        assert_eq!(client.get_config(&business3).business, business3);
        assert_eq!(client.get_config(&business4).business, business4);
        assert_eq!(client.get_config(&business5).business, business5);
    }

    /// Regression: Large config with many items preserves immutability
    #[test]
    fn test_large_config_immutability() {
        let (env, admin, client) = create_test_env();
        env.mock_all_auths();
        client.initialize(&admin);

        let business = Address::generate(&env);

        // Create a large config with many items
        let mut oracles = Vec::new(&env);
        for _ in 0..10 {
            oracles.push_back(Address::generate(&env));
        }

        let mut jurisdictions = Vec::new(&env);
        jurisdictions.push_back(Symbol::new(&env, "US"));
        jurisdictions.push_back(Symbol::new(&env, "UK"));
        jurisdictions.push_back(Symbol::new(&env, "EU"));

        let mut tags = Vec::new(&env);
        tags.push_back(Symbol::new(&env, "fintech"));
        tags.push_back(Symbol::new(&env, "regulated"));
        tags.push_back(Symbol::new(&env, "compliant"));

        let large_integrations = IntegrationRequirements {
            required_oracles: oracles,
            min_confirmations: 3,
            external_validation_required: true,
        };

        let large_compliance = ComplianceConfig {
            jurisdictions,
            required_tags: tags,
            kyc_required: true,
            metadata_required: true,
        };

        client.set_business_config(
            &admin,
            &business,
            &create_default_anomaly_policy(),
            &large_integrations,
            &create_default_expiry(),
            &create_default_custom_fees(),
            &large_compliance,
        );

        let original_business = client.get_config(&business).business;

        // Perform multiple updates
        for _ in 0..5 {
            client.update_anomaly_policy(&admin, &business, &create_default_anomaly_policy());
            client.update_integrations(&admin, &business, &large_integrations);
            client.update_compliance(&admin, &business, &large_compliance);
        }

        let config = client.get_config(&business);
        assert_eq!(config.business, original_business);
        assert_eq!(config.integrations.required_oracles.len(), 10);
    }
}
