use crate::dispute::{DisputeOutcome, DisputeStatus, DisputeType, OptionalResolution};
use crate::test::*;
use crate::{AttestationContract, AttestationContractClient};
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{vec, Address, BytesN, Env, String};

#[test]
fn test_revocation_by_admin() {
    let test = TestEnv::new();
    let admin = test.admin.clone();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-02");
    let merkle_root = BytesN::from_array(&test.env, &[1; 32]);
    let reason = String::from_str(&test.env, "Administrative revocation for audit");

    // Submit an attestation first
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1234567890,
        1,
    );

    // Verify attestation is active
    assert!(!test.is_revoked(business.clone(), period.clone()));
    assert!(test.verify_attestation(business.clone(), period.clone(), &merkle_root));

    // Admin revokes the attestation
    test.revoke_attestation(
        admin.clone(),
        business.clone(),
        period.clone(),
        reason.clone(),
    );

    // Verify revocation
    assert!(test.is_revoked(business.clone(), period.clone()));
    assert!(!test.verify_attestation(business.clone(), period.clone(), &merkle_root));

    // Check revocation details
    let revocation_info = test.get_revocation_info(business.clone(), period.clone());
    assert!(revocation_info.is_some());
    let (revoked_by, _timestamp, returned_reason) = revocation_info.unwrap();
    assert_eq!(revoked_by, admin);
    assert_eq!(returned_reason, reason);

    // Verify attestation data is still preserved
    let attestation = test.get_attestation(business.clone(), period.clone());
    assert!(attestation.is_some());
    let (stored_root, stored_timestamp, stored_version, _stored_fee, _stored_proof, _stored_expiry) =
        attestation.unwrap();
    let (_stored_root, _stored_timestamp, _stored_version, _stored_fee, _stored_expiry) =
        attestation.clone().unwrap();
    let (stored_root, stored_timestamp, stored_version, _stored_fee, _) = attestation.unwrap();
    assert_eq!(stored_root, merkle_root);
    assert_eq!(stored_timestamp, 1234567890);
    assert_eq!(stored_version, 1);
}

#[test]
fn test_revocation_by_business_owner() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-03");
    let merkle_root = BytesN::from_array(&test.env, &[2; 32]);
    let reason = String::from_str(&test.env, "Business correction");

    // Submit an attestation
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1234567891,
        1,
    );

    // Business owner revokes their own attestation
    test.revoke_attestation(
        business.clone(),
        business.clone(),
        period.clone(),
        reason.clone(),
    );

    // Verify revocation
    assert!(test.is_revoked(business.clone(), period.clone()));

    let revocation_info = test.get_revocation_info(business.clone(), period.clone());
    assert!(revocation_info.is_some());
    let (revoked_by, _, returned_reason) = revocation_info.unwrap();
    assert_eq!(revoked_by, business);
    assert_eq!(returned_reason, reason);
}

#[test]
#[should_panic(expected = "caller must be ADMIN or the business owner")]
fn test_unauthorized_revocation() {
    let test = TestEnv::new();
    let unauthorized = Address::generate(&test.env);
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-04");
    let merkle_root = BytesN::from_array(&test.env, &[3; 32]);
    let reason = String::from_str(&test.env, "Unauthorized attempt");

    // Submit an attestation
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1234567892,
        1,
    );

    // Unauthorized user tries to revoke - should panic
    test.revoke_attestation(unauthorized, business.clone(), period.clone(), reason);
}

#[test]
#[should_panic(expected = "attestation not found")]
fn test_revoke_nonexistent_attestation() {
    let test = TestEnv::new();
    let admin = test.admin.clone();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-05");
    let reason = String::from_str(&test.env, "Revoking non-existent");

    // Try to revoke non-existent attestation
    test.revoke_attestation(admin, business.clone(), period.clone(), reason);
}

#[test]
#[should_panic(expected = "attestation already revoked")]
fn test_double_revocation() {
    let test = TestEnv::new();
    let admin = test.admin.clone();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-06");
    let merkle_root = BytesN::from_array(&test.env, &[4; 32]);
    let reason1 = String::from_str(&test.env, "First revocation");
    let reason2 = String::from_str(&test.env, "Second revocation");

    // Submit an attestation
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1234567893,
        1,
    );

    // First revocation
    test.revoke_attestation(admin.clone(), business.clone(), period.clone(), reason1);

    // Try to revoke again - should panic
    test.revoke_attestation(admin, business.clone(), period.clone(), reason2);
}

#[test]
fn test_revocation_preserves_data() {
    let test = TestEnv::new();
    let admin = test.admin.clone();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-07");
    let merkle_root = BytesN::from_array(&test.env, &[5; 32]);
    let timestamp = 1234567894;
    let version = 2;
    let reason = String::from_str(&test.env, "Data preservation test");

    // Submit attestation
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        timestamp,
        version,
    );

    // Get attestation with status before revocation
    let with_status_before = test.get_attestation_with_status(business.clone(), period.clone());
    assert!(with_status_before.is_some());
    let (attestation_data, revocation_info_before) = with_status_before.unwrap();
    assert_eq!(
        attestation_data,
        (merkle_root, timestamp, version, 0, None, None)
    ); // fee = 0, no proof_hash, no expiry in test
    assert_eq!(attestation_data, (merkle_root, timestamp, version, 0, None)); // fee = 0 in test
    assert!(revocation_info_before.is_none());

    // Revoke
    test.revoke_attestation(
        admin.clone(),
        business.clone(),
        period.clone(),
        reason.clone(),
    );

    // Verify data is preserved after revocation
    let with_status_after = test.get_attestation_with_status(business.clone(), period.clone());
    assert!(with_status_after.is_some());
    let (attestation_data_after, revocation_info_after) = with_status_after.unwrap();

    // Attestation data should be identical
    assert_eq!(attestation_data_after, attestation_data);

    // Revocation info should now be present
    assert!(revocation_info_after.is_some());
    let (revoked_by, _revocation_timestamp, returned_reason) = revocation_info_after.unwrap();
    assert_eq!(revoked_by, admin);
    assert_eq!(returned_reason, reason);
}

#[test]
fn test_business_attestations_query() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);

    let periods = vec![
        &test.env,
        String::from_str(&test.env, "2026-01"),
        String::from_str(&test.env, "2026-02"),
        String::from_str(&test.env, "2026-03"),
    ];

    let merkle_roots = [
        BytesN::from_array(&test.env, &[6; 32]),
        BytesN::from_array(&test.env, &[7; 32]),
        BytesN::from_array(&test.env, &[8; 32]),
    ];

    // Submit three attestations
    for i in 0..3 {
        test.submit_attestation(
            business.clone(),
            periods.get(i).unwrap().clone(),
            merkle_roots[i as usize].clone(),
            1234567890 + i as u64,
            1,
        );
    }

    // Revoke the middle one
    test.revoke_attestation(
        test.admin.clone(),
        business.clone(),
        periods.get(1).unwrap().clone(),
        String::from_str(&test.env, "Middle revocation"),
    );

    // Query all attestations
    let results = test.get_business_attestations(business.clone(), periods.clone());
    assert_eq!(results.len(), 3);

    // Check first attestation (active)
    let (period1, attestation1, revocation1) = results.get(0).unwrap();
    assert_eq!(period1, periods.get(0).unwrap());
    assert!(attestation1.is_some());
    assert!(revocation1.is_none());

    // Check second attestation (revoked)
    let (period2, attestation2, revocation2) = results.get(1).unwrap();
    assert_eq!(period2, periods.get(1).unwrap());
    assert!(attestation2.is_some());
    assert!(revocation2.is_some());

    // Check third attestation (active)
    let (period3, attestation3, revocation3) = results.get(2).unwrap();
    assert_eq!(period3, periods.get(2).unwrap());
    assert!(attestation3.is_some());
    assert!(revocation3.is_none());
}

#[test]
fn test_revocation_events() {
    let test = TestEnv::new();
    let admin = test.admin.clone();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-08");
    let merkle_root = BytesN::from_array(&test.env, &[9; 32]);
    let reason = String::from_str(&test.env, "Event test");

    // Submit attestation
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1234567895,
        1,
    );

    // Admin revokes the attestation
    test.revoke_attestation(
        admin.clone(),
        business.clone(),
        period.clone(),
        reason.clone(),
    );

    // Verify the revocation event was emitted
    let events = test.env.events().all();
    assert!(!events.is_empty()); // At least the revocation event
}

#[test]
#[should_panic(expected = "contract is paused")]
fn test_revocation_when_paused() {
    let test = TestEnv::new();
    let admin = test.admin.clone();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-09");
    let merkle_root = BytesN::from_array(&test.env, &[10; 32]);

    // Submit attestation
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1234567896,
        1,
    );

    // Pause the contract
    test.pause(admin.clone());

    // Try to revoke while paused - should panic
    test.revoke_attestation(
        admin,
        business.clone(),
        period.clone(),
        String::from_str(&test.env, "Should fail"),
    );
}

#[test]
fn test_edge_case_empty_reason() {
    let test = TestEnv::new();
    let admin = test.admin.clone();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-10");
    let merkle_root = BytesN::from_array(&test.env, &[11; 32]);
    let empty_reason = String::from_str(&test.env, "");

    // Submit attestation
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1234567897,
        1,
    );

    // Revoke with empty reason (should be allowed)
    test.revoke_attestation(
        admin.clone(),
        business.clone(),
        period.clone(),
        empty_reason.clone(),
    );

    // Verify revocation with empty reason
    let revocation_info = test.get_revocation_info(business.clone(), period.clone());
    assert!(revocation_info.is_some());
    let (_, _, returned_reason) = revocation_info.unwrap();
    assert_eq!(returned_reason, empty_reason);
}

#[test]
fn test_integration_end_to_end_revocation_flow() {
    let test = TestEnv::new();
    let admin = test.admin.clone();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-11");
    let merkle_root = BytesN::from_array(&test.env, &[12; 32]);
    let new_merkle_root = BytesN::from_array(&test.env, &[13; 32]);
    let revoke_reason = String::from_str(&test.env, "End-to-end test");

    // Step 1: Submit initial attestation
    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1234567898,
        1,
    );

    // Step 2: Verify initial state
    assert!(!test.is_revoked(business.clone(), period.clone()));
    assert!(test.verify_attestation(business.clone(), period.clone(), &merkle_root));
    let initial_data = test.get_attestation(business.clone(), period.clone());
    assert!(initial_data.is_some());

    // Step 3: Migrate attestation (admin operation)
    test.migrate_attestation(
        admin.clone(),
        business.clone(),
        period.clone(),
        new_merkle_root.clone(),
        2,
    );

    // Step 4: Verify migration
    assert!(!test.is_revoked(business.clone(), period.clone()));
    assert!(!test.verify_attestation(business.clone(), period.clone(), &merkle_root)); // Old root fails
    assert!(test.verify_attestation(business.clone(), period.clone(), &new_merkle_root)); // New root passes

    // Step 5: Business owner revokes the migrated attestation
    test.revoke_attestation(
        business.clone(),
        business.clone(),
        period.clone(),
        revoke_reason.clone(),
    );

    // Step 6: Verify final state
    assert!(test.is_revoked(business.clone(), period.clone()));
    assert!(!test.verify_attestation(business.clone(), period.clone(), &new_merkle_root));

    // Step 7: Verify data integrity throughout the lifecycle
    let final_data = test.get_attestation(business.clone(), period.clone());
    assert!(final_data.is_some());

    let revocation_info = test.get_revocation_info(business.clone(), period.clone());
    assert!(revocation_info.is_some());
    let (revoked_by, _timestamp, reason) = revocation_info.unwrap();
    assert_eq!(revoked_by, business);
    assert_eq!(reason, revoke_reason);

    // Step 8: Comprehensive status check
    let with_status = test.get_attestation_with_status(business.clone(), period.clone());
    assert!(with_status.is_some());
    let (attestation_data, revocation_data) = with_status.unwrap();
    assert_eq!(attestation_data.0, new_merkle_root); // Should have migrated root
    assert_eq!(attestation_data.2, 2); // Should have migrated version
    assert!(revocation_data.is_some());
}

// ============================================================================
// REVOCATION/DISPUTE STATE TRANSITION TESTS
// ============================================================================

/// Helper to set up contract with dispute capabilities
fn setup_dispute_env() -> (Env, AttestationContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn test_dispute_on_revoked_attestation_fails() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q1");
    let root = BytesN::from_array(&env, &[20; 32]);

    // Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    // Revoke it
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Revocation before dispute"),
    );

    // Attempt to open dispute on revoked attestation - should fail
    let challenger = Address::generate(&env);
    let result = client.try_open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::RevenueMismatch,
        &String::from_str(&env, "Attempting dispute on revoked"),
    );

    // Dispute should fail since attestation is revoked
    assert!(result.is_err());
}

#[test]
fn test_revocation_with_open_dispute() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q2");
    let root = BytesN::from_array(&env, &[21; 32]);

    // Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    // Open dispute
    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::DataIntegrity,
        &String::from_str(&env, "Data integrity concern"),
    );

    // Verify dispute is open
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.status, DisputeStatus::Open);

    // Admin revokes attestation while dispute is open
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Revocation with active dispute"),
    );

    // Verify attestation is revoked
    assert!(client.is_revoked(&business, &period));

    // Dispute should still exist and be queryable
    let dispute_after = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute_after.id, dispute_id);
    assert_eq!(dispute_after.status, DisputeStatus::Open);
}

#[test]
fn test_revocation_with_resolved_dispute() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q3");
    let root = BytesN::from_array(&env, &[22; 32]);

    // Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    // Open and resolve dispute
    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::RevenueMismatch,
        &String::from_str(&env, "Revenue discrepancy"),
    );

    // Resolve dispute
    let resolver = Address::generate(&env);
    client.resolve_dispute(
        &dispute_id,
        &resolver,
        &DisputeOutcome::Rejected,
        &String::from_str(&env, "Attestation verified correct"),
    );

    // Verify dispute is resolved
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.status, DisputeStatus::Resolved);

    // Revoke attestation after dispute resolution
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Post-dispute revocation"),
    );

    // Verify both states
    assert!(client.is_revoked(&business, &period));
    let dispute_final = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute_final.status, DisputeStatus::Resolved);
}

#[test]
fn test_dispute_lifecycle_then_revocation() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-Q4");
    let root = BytesN::from_array(&env, &[23; 32]);

    // Step 1: Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);
    assert!(!client.is_revoked(&business, &period));

    // Step 2: Open dispute
    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::DataIntegrity,
        &String::from_str(&env, "Full lifecycle test"),
    );
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.status, DisputeStatus::Open);

    // Step 3: Resolve dispute (upheld - challenger wins)
    let resolver = Address::generate(&env);
    client.resolve_dispute(
        &dispute_id,
        &resolver,
        &DisputeOutcome::Upheld,
        &String::from_str(&env, "Challenger evidence valid"),
    );
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.status, DisputeStatus::Resolved);

    // Step 4: Close dispute
    client.close_dispute(&dispute_id);
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.status, DisputeStatus::Closed);

    // Step 5: Revoke attestation after complete dispute lifecycle
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Revocation after dispute upheld"),
    );

    // Final verification
    assert!(client.is_revoked(&business, &period));
    let final_dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(final_dispute.status, DisputeStatus::Closed);
}

#[test]
fn test_multiple_challengers_then_revocation() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-05");
    let root = BytesN::from_array(&env, &[24; 32]);

    // Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    // Multiple challengers open disputes
    let challenger1 = Address::generate(&env);
    let challenger2 = Address::generate(&env);

    let dispute_id1 = client.open_dispute(
        &challenger1,
        &business,
        &period,
        &DisputeType::RevenueMismatch,
        &String::from_str(&env, "Challenger 1 dispute"),
    );

    let dispute_id2 = client.open_dispute(
        &challenger2,
        &business,
        &period,
        &DisputeType::DataIntegrity,
        &String::from_str(&env, "Challenger 2 dispute"),
    );

    // Verify both disputes exist
    let disputes = client.get_disputes_by_attestation(&business, &period);
    assert_eq!(disputes.len(), 2);
    assert!(disputes.contains(dispute_id1));
    assert!(disputes.contains(dispute_id2));

    // Revoke attestation with multiple open disputes
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Multiple disputes revocation"),
    );

    // Verify revocation and disputes preserved
    assert!(client.is_revoked(&business, &period));

    let dispute1 = client.get_dispute(&dispute_id1).unwrap();
    let dispute2 = client.get_dispute(&dispute_id2).unwrap();
    assert_eq!(dispute1.status, DisputeStatus::Open);
    assert_eq!(dispute2.status, DisputeStatus::Open);

    // Disputes should still be queryable by attestation
    let disputes_after = client.get_disputes_by_attestation(&business, &period);
    assert_eq!(disputes_after.len(), 2);
}

#[test]
fn test_dispute_resolution_after_revocation() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-06");
    let root = BytesN::from_array(&env, &[25; 32]);

    // Submit attestation and open dispute
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::RevenueMismatch,
        &String::from_str(&env, "Pre-revocation dispute"),
    );

    // Revoke attestation
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Revocation before resolution"),
    );

    // Resolve dispute after revocation - should still work
    let resolver = Address::generate(&env);
    client.resolve_dispute(
        &dispute_id,
        &resolver,
        &DisputeOutcome::Settled,
        &String::from_str(&env, "Settled post-revocation"),
    );

    // Verify final state
    assert!(client.is_revoked(&business, &period));
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.status, DisputeStatus::Resolved);
    if let OptionalResolution::Some(resolution) = dispute.resolution {
        assert_eq!(resolution.outcome, DisputeOutcome::Settled);
    } else {
        panic!("Expected resolution to be present");
    }
}

#[test]
fn test_revocation_preserves_dispute_history() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-07");
    let root = BytesN::from_array(&env, &[26; 32]);

    // Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    // Create and close a dispute
    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::Other,
        &String::from_str(&env, "Historical dispute"),
    );

    let resolver = Address::generate(&env);
    client.resolve_dispute(
        &dispute_id,
        &resolver,
        &DisputeOutcome::Rejected,
        &String::from_str(&env, "Rejected"),
    );
    client.close_dispute(&dispute_id);

    // Record dispute state before revocation
    let dispute_before = client.get_dispute(&dispute_id).unwrap();
    let challenger_disputes_before = client.get_disputes_by_challenger(&challenger);
    let attestation_disputes_before = client.get_disputes_by_attestation(&business, &period);

    // Revoke attestation
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Post-history revocation"),
    );

    // Verify dispute history is preserved after revocation
    let dispute_after = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute_after.id, dispute_before.id);
    assert_eq!(dispute_after.challenger, dispute_before.challenger);
    assert_eq!(dispute_after.status, DisputeStatus::Closed);

    let challenger_disputes_after = client.get_disputes_by_challenger(&challenger);
    assert_eq!(challenger_disputes_after.len(), challenger_disputes_before.len());

    let attestation_disputes_after = client.get_disputes_by_attestation(&business, &period);
    assert_eq!(attestation_disputes_after.len(), attestation_disputes_before.len());
}

#[test]
fn test_state_consistency_across_operations() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-08");
    let root = BytesN::from_array(&env, &[27; 32]);

    // Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    // Initial state assertions
    assert!(!client.is_revoked(&business, &period));
    assert!(client.verify_attestation(&business, &period, &root));
    let disputes = client.get_disputes_by_attestation(&business, &period);
    assert_eq!(disputes.len(), 0);

    // Open dispute
    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::DataIntegrity,
        &String::from_str(&env, "State test"),
    );

    // State after dispute opened
    assert!(!client.is_revoked(&business, &period));
    assert!(client.verify_attestation(&business, &period, &root));
    let disputes = client.get_disputes_by_attestation(&business, &period);
    assert_eq!(disputes.len(), 1);
    assert_eq!(client.get_dispute(&dispute_id).unwrap().status, DisputeStatus::Open);

    // Revoke attestation
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "State transition"),
    );

    // State after revocation
    assert!(client.is_revoked(&business, &period));
    assert!(!client.verify_attestation(&business, &period, &root));
    let disputes = client.get_disputes_by_attestation(&business, &period);
    assert_eq!(disputes.len(), 1);
    assert_eq!(client.get_dispute(&dispute_id).unwrap().status, DisputeStatus::Open);

    // Resolve and close dispute
    let resolver = Address::generate(&env);
    client.resolve_dispute(
        &dispute_id,
        &resolver,
        &DisputeOutcome::Upheld,
        &String::from_str(&env, "Final resolution"),
    );
    client.close_dispute(&dispute_id);

    // Final state verification
    assert!(client.is_revoked(&business, &period));
    assert!(!client.verify_attestation(&business, &period, &root));
    assert_eq!(client.get_dispute(&dispute_id).unwrap().status, DisputeStatus::Closed);
    let disputes = client.get_disputes_by_attestation(&business, &period);
    assert_eq!(disputes.len(), 1);
}

#[test]
fn test_revocation_different_periods_independent() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period1 = String::from_str(&env, "2026-09");
    let period2 = String::from_str(&env, "2026-10");
    let root1 = BytesN::from_array(&env, &[28; 32]);
    let root2 = BytesN::from_array(&env, &[29; 32]);

    // Submit two attestations
    client.submit_attestation(&business, &period1, &root1, &1700000000u64, &1u32, &None, &0u64);
    client.submit_attestation(&business, &period2, &root2, &1700000001u64, &1u32, &None, &0u64);

    // Open dispute on period1
    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period1,
        &DisputeType::RevenueMismatch,
        &String::from_str(&env, "Period 1 dispute"),
    );

    // Revoke period2 (different from disputed period)
    client.revoke_attestation(
        &business,
        &business,
        &period2,
        &String::from_str(&env, "Period 2 revocation"),
    );

    // Verify states are independent
    assert!(!client.is_revoked(&business, &period1));
    assert!(client.is_revoked(&business, &period2));

    // Dispute on period1 should be unaffected
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.status, DisputeStatus::Open);

    // Period2 should have no disputes
    let disputes_period2 = client.get_disputes_by_attestation(&business, &period2);
    assert_eq!(disputes_period2.len(), 0);
}

#[test]
fn test_dispute_outcome_upheld_then_revoke() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-11");
    let root = BytesN::from_array(&env, &[30; 32]);

    // Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    // Open dispute
    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::DataIntegrity,
        &String::from_str(&env, "Upheld dispute scenario"),
    );

    // Resolve as upheld (challenger wins)
    let resolver = Address::generate(&env);
    client.resolve_dispute(
        &dispute_id,
        &resolver,
        &DisputeOutcome::Upheld,
        &String::from_str(&env, "Challenger provided valid evidence"),
    );

    // Verify dispute resolution
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.status, DisputeStatus::Resolved);
    if let OptionalResolution::Some(resolution) = &dispute.resolution {
        assert_eq!(resolution.outcome, DisputeOutcome::Upheld);
    }

    // Business revokes attestation following upheld dispute
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Revoked after dispute upheld"),
    );

    // Final state: both revoked and dispute upheld
    assert!(client.is_revoked(&business, &period));
    let revocation_info = client.get_revocation_info(&business, &period);
    assert!(revocation_info.is_some());
}

#[test]
fn test_closed_dispute_no_reopen_after_revoke() {
    let (env, client, _admin) = setup_dispute_env();
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-12");
    let root = BytesN::from_array(&env, &[31; 32]);

    // Submit attestation
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &0u64);

    // Complete dispute lifecycle
    let challenger = Address::generate(&env);
    let dispute_id = client.open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::RevenueMismatch,
        &String::from_str(&env, "First dispute"),
    );

    let resolver = Address::generate(&env);
    client.resolve_dispute(
        &dispute_id,
        &resolver,
        &DisputeOutcome::Rejected,
        &String::from_str(&env, "Rejected"),
    );
    client.close_dispute(&dispute_id);

    // Revoke attestation
    client.revoke_attestation(
        &business,
        &business,
        &period,
        &String::from_str(&env, "Post-dispute revocation"),
    );

    // Same challenger cannot open new dispute on revoked attestation
    let result = client.try_open_dispute(
        &challenger,
        &business,
        &period,
        &DisputeType::DataIntegrity,
        &String::from_str(&env, "Attempted reopen"),
    );
    assert!(result.is_err());

    // Different challenger also cannot dispute revoked attestation
    let challenger2 = Address::generate(&env);
    let result2 = client.try_open_dispute(
        &challenger2,
        &business,
        &period,
        &DisputeType::Other,
        &String::from_str(&env, "New challenger attempt"),
    );
    assert!(result2.is_err());

    // Verify original dispute still intact
    let final_dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(final_dispute.status, DisputeStatus::Closed);
}

