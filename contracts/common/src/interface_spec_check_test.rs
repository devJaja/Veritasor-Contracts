//! # Interface Specification Consistency Check Tests
//!
//! These tests verify that the documented interface specification remains
//! consistent with the actual contract implementations.

use soroban_sdk::{Env, String};

// Import the module under test
use crate::interface_spec_check::{
    get_event_count, get_expected_events, get_expected_methods, get_expected_structs,
    get_method_count, get_struct_count, is_event_documented, is_method_documented,
    is_struct_documented, verify_interface_consistency, VerificationResult,
};

#[test]
fn test_verification_result_new() {
    let env = Env::default();
    let result = VerificationResult::new(&env);

    assert!(result.passed);
    assert_eq!(result.missing_methods.len(), 0);
    assert_eq!(result.undocumented_methods.len(), 0);
    assert_eq!(result.missing_events.len(), 0);
    assert_eq!(result.missing_structs.len(), 0);
    assert_eq!(result.errors.len(), 0);
}

#[test]
fn test_verification_result_add_missing_method() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);

    result.add_missing_method(&env, String::from_str(&env, "test_method"));

    assert!(!result.passed);
    assert_eq!(result.missing_methods.len(), 1);
}

#[test]
fn test_verification_result_add_undocumented_method() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);

    result.add_undocumented_method(&env, String::from_str(&env, "undoc_method"));

    assert!(!result.passed);
    assert_eq!(result.undocumented_methods.len(), 1);
}

#[test]
fn test_verification_result_add_missing_event() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);

    result.add_missing_event(&env, String::from_str(&env, "test_event"));

    assert!(!result.passed);
    assert_eq!(result.missing_events.len(), 1);
}

#[test]
fn test_verification_result_add_missing_struct() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);

    result.add_missing_struct(&env, String::from_str(&env, "TestStruct"));

    assert!(!result.passed);
    assert_eq!(result.missing_structs.len(), 1);
}

#[test]
fn test_verification_result_add_error() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);

    result.add_error(&env, String::from_str(&env, "test error"));

    assert!(!result.passed);
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn test_get_expected_methods_non_empty() {
    let env = Env::default();
    let methods = get_expected_methods(&env);

    assert!(!methods.is_empty(), "Expected methods should not be empty");
}

#[test]
fn test_get_expected_events_non_empty() {
    let env = Env::default();
    let events = get_expected_events(&env);

    assert!(!events.is_empty(), "Expected events should not be empty");
}

#[test]
fn test_get_expected_structs_non_empty() {
    let env = Env::default();
    let structs = get_expected_structs(&env);

    assert!(!structs.is_empty(), "Expected structs should not be empty");
}

#[test]
fn test_method_count() {
    let env = Env::default();
    let count = get_method_count(&env);

    // Total: 83 methods across all contracts
    assert_eq!(count, 83, "Total method count should be 83");
}

#[test]
fn test_event_count() {
    let env = Env::default();
    let count = get_event_count(&env);

    // Total: 13 events
    assert_eq!(count, 13, "Total event count should be 13");
}

#[test]
fn test_struct_count() {
    let env = Env::default();
    let count = get_struct_count(&env);

    // Total: 17 structs
    assert_eq!(count, 17, "Total struct count should be 17");
}

#[test]
fn test_is_method_documented() {
    let env = Env::default();

    assert!(
        is_method_documented(&env, "AttestationContract", "initialize"),
        "initialize should be documented for AttestationContract"
    );
    assert!(
        is_method_documented(&env, "AttestationContract", "submit_attestation"),
        "submit_attestation should be documented"
    );
    assert!(
        is_method_documented(&env, "IntegrationRegistryContract", "register_provider"),
        "register_provider should be documented"
    );
    assert!(
        !is_method_documented(&env, "AttestationContract", "nonexistent_method"),
        "nonexistent_method should not be documented"
    );
}

#[test]
fn test_is_event_documented() {
    let env = Env::default();

    assert!(
        is_event_documented(&env, "AttestationContract", "AttestationSubmitted"),
        "AttestationSubmitted should be documented"
    );
    assert!(
        is_event_documented(&env, "AttestationContract", "RoleGranted"),
        "RoleGranted should be documented"
    );
    assert!(
        is_event_documented(&env, "IntegrationRegistryContract", "ProviderRegistered"),
        "ProviderRegistered should be documented"
    );
    assert!(
        !is_event_documented(&env, "AttestationContract", "NonexistentEvent"),
        "NonexistentEvent should not be documented"
    );
}

#[test]
fn test_is_struct_documented() {
    let env = Env::default();

    assert!(
        is_struct_documented(&env, "AttestationContract", "FeeConfig"),
        "FeeConfig should be documented"
    );
    assert!(
        is_struct_documented(&env, "AttestationContract", "Proposal"),
        "Proposal should be documented"
    );
    assert!(
        is_struct_documented(&env, "IntegrationRegistryContract", "Provider"),
        "Provider should be documented"
    );
    assert!(
        !is_struct_documented(&env, "AttestationContract", "NonexistentStruct"),
        "NonexistentStruct should not be documented"
    );
}

#[test]
fn test_verify_interface_consistency() {
    let env = Env::default();
    let result = verify_interface_consistency(&env);

    assert!(
        result.passed,
        "Interface consistency verification should pass"
    );
}

#[test]
fn test_all_contracts_have_initialize() {
    let env = Env::default();
    let methods = get_expected_methods(&env);

    let contracts = [
        "AttestationContract",
        "AggregatedAttestationsContract",
        "AttestationSnapshotContract",
        "AuditLogContract",
        "IntegrationRegistryContract",
        "RevenueStreamContract",
    ];

    for contract in contracts.iter() {
        let has_initialize = methods.iter().any(|m| {
            m.contract == String::from_str(&env, contract)
                && m.name == String::from_str(&env, "initialize")
        });
        assert!(has_initialize, "{} should have initialize", contract);
    }
}

#[test]
fn test_all_contracts_have_get_admin() {
    let env = Env::default();
    let methods = get_expected_methods(&env);

    let contracts = [
        "AttestationContract",
        "AggregatedAttestationsContract",
        "AttestationSnapshotContract",
        "AuditLogContract",
        "IntegrationRegistryContract",
        "RevenueStreamContract",
    ];

    for contract in contracts.iter() {
        let has_get_admin = methods.iter().any(|m| {
            m.contract == String::from_str(&env, contract)
                && m.name == String::from_str(&env, "get_admin")
        });
        assert!(has_get_admin, "{} should have get_admin", contract);
    }
}

#[test]
fn test_attestation_events_have_correct_topics() {
    let env = Env::default();
    let events = get_expected_events(&env);

    let expected_topics = [
        ("AttestationSubmitted", "att_sub"),
        ("AttestationRevoked", "att_rev"),
        ("AttestationMigrated", "att_mig"),
        ("RoleGranted", "role_gr"),
        ("RoleRevoked", "role_rv"),
        ("ContractPaused", "paused"),
        ("ContractUnpaused", "unpaus"),
        ("FeeConfigChanged", "fee_cfg"),
    ];

    for (name, expected_topic) in expected_topics.iter() {
        let event = events.iter().find(|e| {
            e.name == String::from_str(&env, name)
                && e.contract == String::from_str(&env, "AttestationContract")
        });
        assert!(event.is_some(), "Event {} should exist", name);
        assert_eq!(
            event.unwrap().topic,
            String::from_str(&env, expected_topic),
            "Event {} should have topic {}",
            name,
            expected_topic
        );
    }
}

#[test]
fn test_provider_events_have_correct_topics() {
    let env = Env::default();
    let events = get_expected_events(&env);

    let expected_topics = [
        ("ProviderRegistered", "prv_reg"),
        ("ProviderEnabled", "prv_ena"),
        ("ProviderDeprecated", "prv_dep"),
        ("ProviderDisabled", "prv_dis"),
        ("ProviderUpdated", "prv_upd"),
    ];

    for (name, expected_topic) in expected_topics.iter() {
        let event = events.iter().find(|e| {
            e.name == String::from_str(&env, name)
                && e.contract == String::from_str(&env, "IntegrationRegistryContract")
        });
        assert!(event.is_some(), "Event {} should exist", name);
        assert_eq!(
            event.unwrap().topic,
            String::from_str(&env, expected_topic),
            "Event {} should have topic {}",
            name,
            expected_topic
        );
    }
}

#[test]
fn test_method_documentation_completeness() {
    let env = Env::default();

    let required_methods = [
        ("AttestationContract", "initialize"),
        ("AttestationContract", "initialize_multisig"),
        ("AttestationContract", "configure_fees"),
        ("AttestationContract", "set_tier_discount"),
        ("AttestationContract", "set_business_tier"),
        ("AttestationContract", "set_volume_brackets"),
        ("AttestationContract", "set_fee_enabled"),
        ("AttestationContract", "grant_role"),
        ("AttestationContract", "revoke_role"),
        ("AttestationContract", "has_role"),
        ("AttestationContract", "get_roles"),
        ("AttestationContract", "get_role_holders"),
        ("AttestationContract", "pause"),
        ("AttestationContract", "unpause"),
        ("AttestationContract", "is_paused"),
        ("AttestationContract", "submit_attestation"),
        ("AttestationContract", "submit_attestation_with_metadata"),
        ("AttestationContract", "revoke_attestation"),
        ("AttestationContract", "migrate_attestation"),
        ("AttestationContract", "is_revoked"),
        ("AttestationContract", "get_attestation"),
        ("AttestationContract", "get_attestation_metadata"),
        ("AttestationContract", "verify_attestation"),
        ("AttestationContract", "create_proposal"),
        ("AttestationContract", "approve_proposal"),
        ("AttestationContract", "reject_proposal"),
        ("AttestationContract", "execute_proposal"),
        ("AttestationContract", "get_proposal"),
        ("AttestationContract", "get_approval_count"),
        ("AttestationContract", "is_proposal_approved"),
        ("AttestationContract", "get_multisig_owners"),
        ("AttestationContract", "get_multisig_threshold"),
        ("AttestationContract", "is_multisig_owner"),
        ("AttestationContract", "get_fee_config"),
        ("AttestationContract", "get_fee_quote"),
        ("AttestationContract", "get_business_tier"),
        ("AttestationContract", "get_business_count"),
        ("AttestationContract", "get_admin"),
    ];

    for (contract, method) in required_methods.iter() {
        assert!(
            is_method_documented(&env, contract, method),
            "Method {}::{} should be documented",
            contract,
            method
        );
    }
}

#[test]
fn test_spec_document_exists() {
    let env = Env::default();
    let method_count = get_method_count(&env);
    assert!(method_count > 0, "Spec should define methods");
}

mod governance_gating_tests {
    use crate::governance_gating::{
        self, get_direct_voting_power, get_emergency_config, get_governance_config,
        get_last_role_assignment, get_role_escalation_config, get_role_escalation_power,
        get_voting_power, has_governance_power, has_role_escalation_power,
        is_emergency_override_admin, is_emergency_paused, record_role_assignment,
        EmergencyConfig, GovernanceConfig, GovernanceKey, RoleEscalationConfig,
    };
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{contract, contractimpl};
    use soroban_sdk::{token, Address, Env};

    #[contract]
    struct GovernanceHarness;

    #[contractimpl]
    impl GovernanceHarness {}

    fn with_harness<T>(env: &Env, harness: &Address, f: impl FnOnce() -> T) -> T {
        env.as_contract(harness, f)
    }

    fn setup_governance(
        threshold: i128,
        enabled: bool,
    ) -> (Env, Address, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let harness = env.register(GovernanceHarness, ());
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
        let token = token_contract.address().clone();

        with_harness(&env, &harness, || {
            governance_gating::initialize_governance(&env, &token, threshold, enabled);
        });

        (env, harness, token, admin, alice, bob)
    }

    fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
        token::StellarAssetClient::new(env, token).mint(to, &amount);
    }

    #[test]
    fn test_initialize_governance_defaults_role_escalation_controls() {
        let (env, harness, token, _admin, _alice, _bob) = setup_governance(100, true);

        with_harness(&env, &harness, || {
            assert_eq!(
                get_governance_config(&env),
                Some(GovernanceConfig {
                    token: token.clone(),
                    threshold: 100,
                    enabled: true,
                })
            );
            assert_eq!(
                get_role_escalation_config(&env),
                Some(RoleEscalationConfig {
                    threshold: 100,
                    allow_delegated_power: false,
                })
            );
            assert!(governance_gating::is_governance_enabled(&env));
        });
    }

    #[test]
    fn test_get_role_escalation_config_returns_none_when_uninitialized() {
        let env = Env::default();
        let harness = env.register(GovernanceHarness, ());

        with_harness(&env, &harness, || {
            assert_eq!(get_governance_config(&env), None);
            assert_eq!(get_role_escalation_config(&env), None);
            assert!(!governance_gating::is_governance_enabled(&env));
        });
    }

    #[test]
    fn test_get_governance_config_returns_none_for_partial_legacy_state_without_threshold() {
        let env = Env::default();
        env.mock_all_auths();

        let harness = env.register(GovernanceHarness, ());
        let admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
        let token = token_contract.address().clone();

        with_harness(&env, &harness, || {
            env.storage()
                .instance()
                .set(&GovernanceKey::GovernanceToken, &token);
            env.storage()
                .instance()
                .set(&GovernanceKey::GovernanceEnabled, &true);

            assert_eq!(get_governance_config(&env), None);
            assert_eq!(get_role_escalation_config(&env), None);
        });
    }

    #[test]
    #[should_panic(expected = "threshold must be non-negative")]
    fn test_initialize_governance_rejects_negative_threshold() {
        let env = Env::default();
        env.mock_all_auths();

        let harness = env.register(GovernanceHarness, ());
        let admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
        let token = token_contract.address().clone();

        with_harness(&env, &harness, || {
            governance_gating::initialize_governance(&env, &token, -1, true);
        });
    }

    #[test]
    #[should_panic(expected = "governance already initialized")]
    fn test_initialize_governance_rejects_reinitialization() {
        let (env, harness, token, _admin, _alice, _bob) = setup_governance(100, true);
        with_harness(&env, &harness, || {
            governance_gating::initialize_governance(&env, &token, 100, true);
        });
    }

    #[test]
    fn test_set_governance_threshold_bumps_role_escalation_floor() {
        let (env, harness, _token, _admin, _alice, _bob) = setup_governance(100, true);

        with_harness(&env, &harness, || {
            governance_gating::set_role_escalation_threshold(&env, 150);
            governance_gating::set_governance_threshold(&env, 200);

            assert_eq!(get_governance_config(&env).unwrap().threshold, 200);
            assert_eq!(get_role_escalation_config(&env).unwrap().threshold, 200);
        });
    }

    #[test]
    fn test_set_governance_threshold_preserves_higher_role_escalation_threshold() {
        let (env, harness, _token, _admin, _alice, _bob) = setup_governance(100, true);

        with_harness(&env, &harness, || {
            governance_gating::set_role_escalation_threshold(&env, 175);
            governance_gating::set_governance_threshold(&env, 120);

            assert_eq!(get_governance_config(&env).unwrap().threshold, 120);
            assert_eq!(get_role_escalation_config(&env).unwrap().threshold, 175);
        });
    }

    #[test]
    #[should_panic(expected = "threshold must be non-negative")]
    fn test_set_governance_threshold_rejects_negative_value() {
        let (env, harness, _token, _admin, _alice, _bob) = setup_governance(100, true);
        with_harness(&env, &harness, || {
            governance_gating::set_governance_threshold(&env, -1);
        });
    }

    #[test]
    #[should_panic(expected = "threshold must be non-negative")]
    fn test_set_role_escalation_threshold_rejects_negative_value() {
        let (env, harness, _token, _admin, _alice, _bob) = setup_governance(100, true);
        with_harness(&env, &harness, || {
            governance_gating::set_role_escalation_threshold(&env, -1);
        });
    }

    #[test]
    #[should_panic(expected = "role escalation threshold must be >= governance threshold")]
    fn test_set_role_escalation_threshold_rejects_lower_value_than_base_governance() {
        let (env, harness, _token, _admin, _alice, _bob) = setup_governance(100, true);
        with_harness(&env, &harness, || {
            governance_gating::set_role_escalation_threshold(&env, 99);
        });
    }

    #[test]
    #[should_panic(expected = "governance not initialized")]
    fn test_set_role_escalation_use_delegated_power_requires_initialization() {
        let env = Env::default();
        let harness = env.register(GovernanceHarness, ());
        with_harness(&env, &harness, || {
            governance_gating::set_role_escalation_use_delegated_power(&env, true);
        });
    }

    #[test]
    fn test_set_governance_enabled_toggles_enabled_flag() {
        let (env, harness, _token, _admin, alice, _bob) = setup_governance(100, true);

        with_harness(&env, &harness, || {
            assert!(governance_gating::is_governance_enabled(&env));
            governance_gating::set_governance_enabled(&env, false);

            assert!(!governance_gating::is_governance_enabled(&env));
            assert!(!has_governance_power(&env, &alice));

            governance_gating::set_governance_enabled(&env, true);
            assert!(governance_gating::is_governance_enabled(&env));
        });
    }

    #[test]
    fn test_get_direct_and_total_voting_power_return_zero_when_governance_uninitialized() {
        let env = Env::default();
        let harness = env.register(GovernanceHarness, ());
        let alice = Address::generate(&env);

        with_harness(&env, &harness, || {
            assert_eq!(get_direct_voting_power(&env, &alice), 0);
            assert_eq!(get_voting_power(&env, &alice), 0);
            assert_eq!(get_role_escalation_power(&env, &alice), 0);
        });
    }

    #[test]
    fn test_delegate_voting_power_transfers_power_without_duplication() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(100, true);
        mint(&env, &token, &alice, 100);
        mint(&env, &token, &bob, 50);

        with_harness(&env, &harness, || {
            assert_eq!(get_direct_voting_power(&env, &alice), 100);
            assert_eq!(get_voting_power(&env, &alice), 100);
            assert_eq!(get_voting_power(&env, &bob), 50);

            governance_gating::delegate_voting_power(&env, &alice, &bob);

            assert_eq!(
                governance_gating::get_delegate(&env, &alice),
                Some(bob.clone())
            );
            assert_eq!(get_voting_power(&env, &alice), 0);
            assert_eq!(get_voting_power(&env, &bob), 150);
            assert!(!has_governance_power(&env, &alice));
            assert!(has_governance_power(&env, &bob));
        });
    }

    #[test]
    #[should_panic(expected = "governance not initialized")]
    fn test_delegate_voting_power_requires_initialization() {
        let env = Env::default();
        env.mock_all_auths();

        let harness = env.register(GovernanceHarness, ());
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        with_harness(&env, &harness, || {
            governance_gating::delegate_voting_power(&env, &alice, &bob);
        });
    }

    #[test]
    #[should_panic(expected = "cannot delegate to self")]
    fn test_delegate_voting_power_rejects_self_delegation() {
        let (env, harness, token, _admin, alice, _bob) = setup_governance(100, true);
        mint(&env, &token, &alice, 100);
        with_harness(&env, &harness, || {
            governance_gating::delegate_voting_power(&env, &alice, &alice);
        });
    }

    #[test]
    fn test_redelegation_uses_current_balance_and_removes_old_snapshot_from_previous_delegate() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(100, true);
        let carol = Address::generate(&env);
        mint(&env, &token, &alice, 100);
        mint(&env, &token, &bob, 50);
        mint(&env, &token, &carol, 25);

        with_harness(&env, &harness, || {
            governance_gating::delegate_voting_power(&env, &alice, &bob);
        });
        mint(&env, &token, &alice, 40);

        with_harness(&env, &harness, || {
            governance_gating::delegate_voting_power(&env, &alice, &carol);

            assert_eq!(
                governance_gating::get_delegate(&env, &alice),
                Some(carol.clone())
            );
            assert_eq!(get_voting_power(&env, &alice), 0);
            assert_eq!(get_voting_power(&env, &bob), 50);
            assert_eq!(get_voting_power(&env, &carol), 165);
        });
    }

    #[test]
    fn test_revoke_delegation_reconciles_snapshotted_power_and_restores_direct_balance() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(100, true);
        mint(&env, &token, &alice, 100);
        mint(&env, &token, &bob, 50);

        with_harness(&env, &harness, || {
            governance_gating::delegate_voting_power(&env, &alice, &bob);
        });
        mint(&env, &token, &alice, 25);
        with_harness(&env, &harness, || {
            governance_gating::revoke_delegation(&env, &alice);

            assert_eq!(governance_gating::get_delegate(&env, &alice), None);
            assert_eq!(get_voting_power(&env, &alice), 125);
            assert_eq!(get_voting_power(&env, &bob), 50);
        });
    }

    #[test]
    fn test_revoke_delegation_is_noop_when_no_delegate_exists() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(100, true);
        mint(&env, &token, &alice, 100);
        mint(&env, &token, &bob, 50);

        with_harness(&env, &harness, || {
            governance_gating::revoke_delegation(&env, &alice);

            assert_eq!(governance_gating::get_delegate(&env, &alice), None);
            assert_eq!(get_voting_power(&env, &alice), 100);
            assert_eq!(get_voting_power(&env, &bob), 50);
        });
    }

    #[test]
    fn test_revoke_delegation_uses_legacy_balance_fallback_when_snapshot_is_missing() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(100, true);
        mint(&env, &token, &alice, 80);
        mint(&env, &token, &bob, 20);

        with_harness(&env, &harness, || {
            env.storage()
                .instance()
                .set(&GovernanceKey::Delegation(alice.clone()), &bob);
            env.storage()
                .instance()
                .set(&GovernanceKey::DelegatedPower(bob.clone()), &80i128);

            governance_gating::revoke_delegation(&env, &alice);

            assert_eq!(governance_gating::get_delegate(&env, &alice), None);
            assert_eq!(get_voting_power(&env, &alice), 80);
            assert_eq!(get_voting_power(&env, &bob), 20);
        });
    }

    #[test]
    fn test_role_escalation_defaults_to_direct_balance_only() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(50, true);
        mint(&env, &token, &alice, 100);
        mint(&env, &token, &bob, 50);

        with_harness(&env, &harness, || {
            governance_gating::set_role_escalation_threshold(&env, 120);
            governance_gating::delegate_voting_power(&env, &alice, &bob);

            assert_eq!(get_role_escalation_power(&env, &bob), 50);
            assert!(has_governance_power(&env, &bob));
            assert!(!has_role_escalation_power(&env, &bob));
        });
    }

    #[test]
    fn test_role_escalation_can_opt_into_delegated_power() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(50, true);
        mint(&env, &token, &alice, 100);
        mint(&env, &token, &bob, 50);

        with_harness(&env, &harness, || {
            governance_gating::set_role_escalation_threshold(&env, 120);
            governance_gating::delegate_voting_power(&env, &alice, &bob);
            governance_gating::set_role_escalation_use_delegated_power(&env, true);

            assert_eq!(get_role_escalation_power(&env, &bob), 150);
            assert!(has_role_escalation_power(&env, &bob));
            governance_gating::require_role_escalation_threshold(&env, &bob);
        });
    }

    #[test]
    fn test_has_governance_and_role_escalation_power_fail_closed_when_uninitialized() {
        let env = Env::default();
        let harness = env.register(GovernanceHarness, ());
        let alice = Address::generate(&env);

        with_harness(&env, &harness, || {
            assert!(!has_governance_power(&env, &alice));
            assert!(!has_role_escalation_power(&env, &alice));
        });
    }

    #[test]
    fn test_has_governance_power_returns_false_when_disabled() {
        let (env, harness, token, _admin, alice, _bob) = setup_governance(100, false);
        mint(&env, &token, &alice, 200);

        with_harness(&env, &harness, || {
            assert!(!has_governance_power(&env, &alice));
            assert!(!has_role_escalation_power(&env, &alice));
        });
    }

    #[test]
    #[should_panic(expected = "governance disabled")]
    fn test_require_role_escalation_threshold_fails_closed_when_governance_disabled() {
        let (env, harness, token, _admin, alice, _bob) = setup_governance(100, false);
        mint(&env, &token, &alice, 200);

        with_harness(&env, &harness, || {
            assert!(!governance_gating::is_governance_enabled(&env));
            assert!(!has_role_escalation_power(&env, &alice));
            governance_gating::require_role_escalation_threshold(&env, &alice);
        });
    }

    #[test]
    fn test_require_governance_threshold_allows_uninitialized_or_disabled_governance() {
        let uninitialized_env = Env::default();
        uninitialized_env.mock_all_auths();
        let uninitialized_harness = uninitialized_env.register(GovernanceHarness, ());
        let caller = Address::generate(&uninitialized_env);
        with_harness(&uninitialized_env, &uninitialized_harness, || {
            governance_gating::require_governance_threshold(&uninitialized_env, &caller);
        });

        let (env, harness, token, _admin, alice, _bob) = setup_governance(100, false);
        mint(&env, &token, &alice, 10);
        with_harness(&env, &harness, || {
            governance_gating::require_governance_threshold(&env, &alice);
        });
    }

    #[test]
    #[should_panic(expected = "insufficient governance voting power")]
    fn test_require_governance_threshold_rejects_insufficient_power_when_enabled() {
        let (env, harness, token, _admin, alice, _bob) = setup_governance(100, true);
        mint(&env, &token, &alice, 99);

        with_harness(&env, &harness, || {
            governance_gating::require_governance_threshold(&env, &alice);
        });
    }

    #[test]
    fn test_legacy_governance_state_falls_back_to_secure_role_escalation_defaults() {
        let env = Env::default();
        env.mock_all_auths();

        let harness = env.register(GovernanceHarness, ());
        let admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
        let token = token_contract.address().clone();

        with_harness(&env, &harness, || {
            env.storage()
                .instance()
                .set(&GovernanceKey::GovernanceToken, &token);
            env.storage()
                .instance()
                .set(&GovernanceKey::GovernanceThreshold, &123i128);
            env.storage()
                .instance()
                .set(&GovernanceKey::GovernanceEnabled, &true);

            assert_eq!(
                get_role_escalation_config(&env),
                Some(RoleEscalationConfig {
                    threshold: 123,
                    allow_delegated_power: false,
                })
            );
        });
    }

    // ════════════════════════════════════════════════════════════════════
    // Emergency Controls Tests
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_emergency_config_defaults_to_safe_state() {
        let (env, harness, _token, _admin, _alice, _bob) = setup_governance(100, true);

        with_harness(&env, &harness, || {
            let emergency = governance_gating::get_emergency_config(&env).unwrap();
            assert!(!emergency.paused);
            assert!(emergency.override_admin.is_none());
            assert!(!governance_gating::is_emergency_paused(&env));
        });
    }

    #[test]
    fn test_emergency_config_returns_none_when_governance_uninitialized() {
        let env = Env::default();
        let harness = env.register(GovernanceHarness, ());

        with_harness(&env, &harness, || {
            assert!(governance_gating::get_emergency_config(&env).is_none());
            assert!(!governance_gating::is_emergency_paused(&env));
        });
    }

    #[test]
    fn test_set_emergency_pause_requires_role_escalation_power_to_activate() {
        let (env, harness, token, _admin, alice, _bob) = setup_governance(50, true);
        mint(&env, &token, &alice, 40); // Below role escalation threshold

        with_harness(&env, &harness, || {
            // Should panic when trying to activate pause without sufficient power
            let result = std::panic::catch_unwind(|| {
                governance_gating::set_emergency_pause(&env, &alice, true);
            });
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_set_emergency_pause_allows_deactivation_without_power_check() {
        let (env, harness, token, _admin, alice, _bob) = setup_governance(50, true);
        mint(&env, &token, &alice, 40); // Below role escalation threshold

        with_harness(&env, &harness, || {
            // First activate pause with sufficient power
            mint(&env, &token, &alice, 60); // Now has 100 total
            governance_gating::set_emergency_pause(&env, &alice, true);
            assert!(governance_gating::is_emergency_paused(&env));

            // Should allow deactivation even with insufficient power
            mint(&env, &token, &alice, -60); // Back to 40
            governance_gating::set_emergency_pause(&env, &alice, false);
            assert!(!governance_gating::is_emergency_paused(&env));
        });
    }

    #[test]
    fn test_emergency_pause_blocks_governance_operations_for_non_override_admin() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(50, true);
        mint(&env, &token, &alice, 100);
        mint(&env, &token, &bob, 100);

        with_harness(&env, &harness, || {
            governance_gating::set_emergency_pause(&env, true);
            assert!(governance_gating::is_emergency_paused(&env));

            // Alice should be blocked
            let result = std::panic::catch_unwind(|| {
                governance_gating::require_governance_threshold(&env, &alice);
            });
            assert!(result.is_err());

            // Bob should also be blocked
            let result = std::panic::catch_unwind(|| {
                governance_gating::require_role_escalation_threshold(&env, &bob);
            });
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_emergency_override_admin_can_bypass_pause() {
        let (env, harness, token, _admin, alice, bob) = setup_governance(50, true);
        mint(&env, &token, &alice, 100);
        mint(&env, &token, &bob, 100);

        with_harness(&env, &harness, || {
            governance_gating::set_emergency_override_admin(&env, &alice, Some(alice.clone()));
            governance_gating::set_emergency_pause(&env, &alice, true);

            assert!(governance_gating::is_emergency_override_admin(&env, &alice));
            assert!(!governance_gating::is_emergency_override_admin(&env, &bob));

            // Alice should be able to bypass pause
            governance_gating::require_governance_threshold(&env, &alice);
            governance_gating::require_role_escalation_threshold(&env, &alice);

            // Bob should still be blocked
            let result = std::panic::catch_unwind(|| {
                governance_gating::require_governance_threshold(&env, &bob);
            });
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_set_emergency_override_admin_requires_role_escalation_power() {
        let (env, harness, token, _admin, alice, _bob) = setup_governance(50, true);
        mint(&env, &token, &alice, 40); // Below threshold

        with_harness(&env, &harness, || {
            let result = std::panic::catch_unwind(|| {
                governance_gating::set_emergency_override_admin(&env, &alice, Some(alice.clone()));
            });
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_clear_emergency_override_admin() {
        let (env, harness, token, _admin, alice, _bob) = setup_governance(50, true);
        mint(&env, &token, &alice, 100);

        with_harness(&env, &harness, || {
            governance_gating::set_emergency_override_admin(&env, &alice, Some(alice.clone()));
            assert!(governance_gating::is_emergency_override_admin(&env, &alice));

            governance_gating::set_emergency_override_admin(&env, &alice, None);
            assert!(!governance_gating::is_emergency_override_admin(&env, &alice));
        });
    }

    // ════════════════════════════════════════════════════════════════════
    // Role Drift Protection Tests
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_role_assignment_tracking() {
        let (env, harness, _token, _admin, alice, _bob) = setup_governance(100, true);

        with_harness(&env, &harness, || {
            assert!(governance_gating::get_last_role_assignment(&env, &alice).is_none());

            governance_gating::record_role_assignment(&env, &alice, 12345);
            assert_eq!(governance_gating::get_last_role_assignment(&env, &alice), Some(12345));

            governance_gating::record_role_assignment(&env, &alice, 67890);
            assert_eq!(governance_gating::get_last_role_assignment(&env, &alice), Some(67890));
        });
    }

    #[test]
    fn test_role_assignment_tracking_isolated_per_address() {
        let (env, harness, _token, _admin, alice, bob) = setup_governance(100, true);

        with_harness(&env, &harness, || {
            governance_gating::record_role_assignment(&env, &alice, 11111);
            governance_gating::record_role_assignment(&env, &bob, 22222);

            assert_eq!(governance_gating::get_last_role_assignment(&env, &alice), Some(11111));
            assert_eq!(governance_gating::get_last_role_assignment(&env, &bob), Some(22222));
        });
    }
}
