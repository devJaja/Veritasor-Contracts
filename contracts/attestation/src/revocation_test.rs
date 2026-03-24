#![cfg(test)]

use crate::events::TOPIC_ATTESTATION_REVOKED;
use crate::test::TestEnv;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{vec, Address, BytesN, IntoVal, String};

#[test]
fn test_revocation_by_admin() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-02");
    let merkle_root = BytesN::from_array(&test.env, &[1; 32]);
    let reason = String::from_str(&test.env, "Administrative revocation for audit");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1_234_567_890,
        1,
    );

    assert!(!test.is_revoked(business.clone(), period.clone()));
    assert!(test.verify_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone()
    ));

    test.revoke_attestation(
        test.admin.clone(),
        business.clone(),
        period.clone(),
        reason.clone(),
    );

    assert!(test.is_revoked(business.clone(), period.clone()));
    assert!(!test.verify_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone()
    ));

    let (revoked_by, _, stored_reason) = test
        .get_revocation_info(business.clone(), period.clone())
        .unwrap();
    assert_eq!(revoked_by, test.admin);
    assert_eq!(stored_reason, reason);

    let (stored_root, stored_timestamp, stored_version, _, stored_proof, stored_expiry) = test
        .get_attestation(business.clone(), period.clone())
        .unwrap();
    assert_eq!(stored_root, merkle_root);
    assert_eq!(stored_timestamp, 1_234_567_890);
    assert_eq!(stored_version, 1);
    assert_eq!(stored_proof, None);
    assert_eq!(stored_expiry, None);
}

#[test]
fn test_revocation_by_business_owner() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-03");
    let reason = String::from_str(&test.env, "Business correction");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        BytesN::from_array(&test.env, &[2; 32]),
        1_234_567_891,
        1,
    );

    test.revoke_attestation(
        business.clone(),
        business.clone(),
        period.clone(),
        reason.clone(),
    );

    let (revoked_by, _, stored_reason) = test.get_revocation_info(business.clone(), period).unwrap();
    assert_eq!(revoked_by, business);
    assert_eq!(stored_reason, reason);
}

#[test]
#[should_panic(expected = "caller must be ADMIN or the business owner")]
fn test_unauthorized_revocation() {
    let test = TestEnv::new();
    let unauthorized = Address::generate(&test.env);
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-04");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        BytesN::from_array(&test.env, &[3; 32]),
        1_234_567_892,
        1,
    );

    test.revoke_attestation(
        unauthorized,
        business,
        period,
        String::from_str(&test.env, "Unauthorized attempt"),
    );
}

#[test]
#[should_panic(expected = "attestation not found")]
fn test_revoke_nonexistent_attestation() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);

    test.revoke_attestation(
        test.admin.clone(),
        business,
        String::from_str(&test.env, "2026-05"),
        String::from_str(&test.env, "Revoking non-existent"),
    );
}

#[test]
#[should_panic(expected = "attestation already revoked")]
fn test_double_revocation_rejected_as_replay() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-06");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        BytesN::from_array(&test.env, &[4; 32]),
        1_234_567_893,
        1,
    );

    test.revoke_attestation(
        test.admin.clone(),
        business.clone(),
        period.clone(),
        String::from_str(&test.env, "First revocation"),
    );

    test.revoke_attestation(
        test.admin.clone(),
        business,
        period,
        String::from_str(&test.env, "Replay revocation"),
    );
}

#[test]
fn test_get_attestation_with_status_preserves_attestation_data() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-07");
    let merkle_root = BytesN::from_array(&test.env, &[5; 32]);
    let reason = String::from_str(&test.env, "Data preservation test");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        merkle_root.clone(),
        1_234_567_894,
        2,
    );

    let (attestation_before, revocation_before) = test
        .get_attestation_with_status(business.clone(), period.clone())
        .unwrap();
    assert_eq!(attestation_before, (merkle_root.clone(), 1_234_567_894, 2, 0, None, None));
    assert_eq!(revocation_before, None);

    test.revoke_attestation(
        test.admin.clone(),
        business.clone(),
        period.clone(),
        reason.clone(),
    );

    let (attestation_after, revocation_after) = test
        .get_attestation_with_status(business, period)
        .unwrap();
    assert_eq!(attestation_after, attestation_before);
    assert_eq!(revocation_after.unwrap().2, reason);
}

#[test]
fn test_get_business_attestations_preserves_order_and_missing_periods() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let periods = vec![
        &test.env,
        String::from_str(&test.env, "2026-01"),
        String::from_str(&test.env, "2026-02"),
        String::from_str(&test.env, "2026-99"),
        String::from_str(&test.env, "2026-03"),
    ];

    test.submit_attestation(
        business.clone(),
        periods.get(0).unwrap().clone(),
        BytesN::from_array(&test.env, &[6; 32]),
        1_234_567_900,
        1,
    );
    test.submit_attestation(
        business.clone(),
        periods.get(1).unwrap().clone(),
        BytesN::from_array(&test.env, &[7; 32]),
        1_234_567_901,
        1,
    );
    test.submit_attestation(
        business.clone(),
        periods.get(3).unwrap().clone(),
        BytesN::from_array(&test.env, &[8; 32]),
        1_234_567_902,
        1,
    );

    test.revoke_attestation(
        test.admin.clone(),
        business.clone(),
        periods.get(1).unwrap().clone(),
        String::from_str(&test.env, "Middle revocation"),
    );

    let results = test.get_business_attestations(business, periods.clone());
    assert_eq!(results.len(), 4);

    let (period0, attestation0, revocation0) = results.get(0).unwrap();
    assert_eq!(period0, periods.get(0).unwrap());
    assert!(attestation0.is_some());
    assert!(revocation0.is_none());

    let (period1, attestation1, revocation1) = results.get(1).unwrap();
    assert_eq!(period1, periods.get(1).unwrap());
    assert!(attestation1.is_some());
    assert!(revocation1.is_some());

    let (period2, attestation2, revocation2) = results.get(2).unwrap();
    assert_eq!(period2, periods.get(2).unwrap());
    assert!(attestation2.is_none());
    assert!(revocation2.is_none());

    let (period3, attestation3, revocation3) = results.get(3).unwrap();
    assert_eq!(period3, periods.get(3).unwrap());
    assert!(attestation3.is_some());
    assert!(revocation3.is_none());
}

#[test]
fn test_revocation_event_emitted() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-08");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        BytesN::from_array(&test.env, &[9; 32]),
        1_234_567_895,
        1,
    );

    test.revoke_attestation(
        test.admin.clone(),
        business.clone(),
        period.clone(),
        String::from_str(&test.env, "Event test"),
    );
    let events = test.env.events().all();
    let expected_topics = (TOPIC_ATTESTATION_REVOKED, business).into_val(&test.env);
    assert!(!events.is_empty());
    assert!(events.iter().any(|event| event.1 == expected_topics));
}

#[test]
#[should_panic(expected = "contract is paused")]
fn test_revocation_when_paused() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-09");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        BytesN::from_array(&test.env, &[10; 32]),
        1_234_567_896,
        1,
    );

    test.pause(test.admin.clone());

    test.revoke_attestation(
        test.admin.clone(),
        business,
        period,
        String::from_str(&test.env, "Should fail"),
    );
}

#[test]
fn test_edge_case_empty_reason() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-10");
    let empty_reason = String::from_str(&test.env, "");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        BytesN::from_array(&test.env, &[11; 32]),
        1_234_567_897,
        1,
    );

    test.revoke_attestation(
        test.admin.clone(),
        business.clone(),
        period.clone(),
        empty_reason.clone(),
    );

    let (_, _, stored_reason) = test.get_revocation_info(business, period).unwrap();
    assert_eq!(stored_reason, empty_reason);
}

#[test]
#[should_panic(expected = "attestation revoked")]
fn test_migration_after_revocation_is_blocked() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-11");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        BytesN::from_array(&test.env, &[12; 32]),
        1_234_567_898,
        1,
    );
    test.revoke_attestation(
        test.admin.clone(),
        business.clone(),
        period.clone(),
        String::from_str(&test.env, "Finalize attestation"),
    );

    test.migrate_attestation(
        test.admin.clone(),
        business,
        period,
        BytesN::from_array(&test.env, &[13; 32]),
        2,
    );
}

#[test]
fn test_integration_migration_then_business_owner_revocation() {
    let test = TestEnv::new();
    let business = Address::generate(&test.env);
    let period = String::from_str(&test.env, "2026-12");
    let original_root = BytesN::from_array(&test.env, &[14; 32]);
    let migrated_root = BytesN::from_array(&test.env, &[15; 32]);
    let revoke_reason = String::from_str(&test.env, "End-to-end test");

    test.submit_attestation(
        business.clone(),
        period.clone(),
        original_root.clone(),
        1_234_567_899,
        1,
    );

    assert!(test.verify_attestation(
        business.clone(),
        period.clone(),
        original_root.clone()
    ));

    test.migrate_attestation(
        test.admin.clone(),
        business.clone(),
        period.clone(),
        migrated_root.clone(),
        2,
    );

    assert!(!test.verify_attestation(
        business.clone(),
        period.clone(),
        original_root
    ));
    assert!(test.verify_attestation(
        business.clone(),
        period.clone(),
        migrated_root.clone()
    ));

    test.revoke_attestation(
        business.clone(),
        business.clone(),
        period.clone(),
        revoke_reason.clone(),
    );

    assert!(test.is_revoked(business.clone(), period.clone()));
    assert!(!test.verify_attestation(business.clone(), period.clone(), migrated_root));

    let (attestation, revocation) = test.get_attestation_with_status(business, period).unwrap();
    assert_eq!(attestation.2, 2);
    assert_eq!(revocation.unwrap().2, revoke_reason);
}
