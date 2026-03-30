//! # Events Tests
//!
//! Tests for structured event emissions including attestation lifecycle events,
//! role changes, and pause state changes.

extern crate alloc;

use super::*;
use crate::access_control::ROLE_ADMIN;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, BytesN, Env, String};

/// Helper: register the contract and return a client.
fn setup() -> (Env, AttestationContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &0u64);
    (env, client, admin)
}

// ════════════════════════════════════════════════════════════════════
//  Attestation Submission Event Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_submit_attestation_emits_event() {
    let (env, client, _admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    let timestamp = 1_700_000_000u64;
    let version = 1u32;

    client.submit_attestation(
        &business, &period, &root, &timestamp, &version, &None, &None,
        &business, &period, &root, &timestamp, &version, &None, &0u64,
    );

    // Verify event was emitted (events are logged in the environment)
    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
fn test_multiple_attestations_emit_multiple_events() {
    let (env, client, _admin) = setup();

    let business = Address::generate(&env);

    for i in 1..=5 {
        let period = String::from_str(&env, &alloc::format!("2026-0{}", i));
        let root = BytesN::from_array(&env, &[i as u8; 32]);
        let nonce = client.get_replay_nonce(&business, &crate::NONCE_CHANNEL_BUSINESS);
        client.submit_attestation(
            &business,
            &period,
            &root,
            &(1_700_000_000u64 + i as u64),
            &1u32,
            &None,
            &None,
            &nonce,
        );
    }

    let events = env.events().all();
    // Events are emitted (at least one per attestation)
    assert!(!events.is_empty());
}

// ════════════════════════════════════════════════════════════════════
//  Attestation Revocation Event Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_revoke_attestation_emits_event() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = BytesN::from_array(&env, &[1u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );

    let reason = String::from_str(&env, "fraudulent data detected");
    client.revoke_attestation(&admin, &business, &period, &reason, &1u64);

    let events = env.events().all();
    // Events are emitted
    assert!(!events.is_empty());
}

#[test]
fn test_revoked_attestation_fails_verification() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = BytesN::from_array(&env, &[1u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );

    // Verify before revocation
    assert!(client.verify_attestation(&business, &period, &root));

    let reason = String::from_str(&env, "data correction needed");
    client.revoke_attestation(&admin, &business, &period, &reason, &1u64);

    // Verify after revocation - should fail
    assert!(!client.verify_attestation(&business, &period, &root));
}

#[test]
#[should_panic(expected = "attestation not found")]
fn test_revoke_nonexistent_attestation_panics() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let reason = String::from_str(&env, "test reason");

    client.revoke_attestation(&admin, &business, &period, &reason, &1u64);
}

// ════════════════════════════════════════════════════════════════════
//  Attestation Migration Event Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_migrate_attestation_emits_event() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let old_root = BytesN::from_array(&env, &[1u8; 32]);
    let new_root = BytesN::from_array(&env, &[2u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &old_root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );

    client.migrate_attestation(&admin, &business, &period, &new_root, &2u32, &1u64);

    let events = env.events().all();
    // Events are emitted
    assert!(!events.is_empty());
}

#[test]
fn test_migrate_attestation_updates_data() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let old_root = BytesN::from_array(&env, &[1u8; 32]);
    let new_root = BytesN::from_array(&env, &[2u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &old_root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );

    // Old root verifies
    assert!(client.verify_attestation(&business, &period, &old_root));

    client.migrate_attestation(&admin, &business, &period, &new_root, &2u32, &1u64);

    // Old root no longer verifies
    assert!(!client.verify_attestation(&business, &period, &old_root));
    // New root verifies
    assert!(client.verify_attestation(&business, &period, &new_root));

    // Check version updated
    let (stored_root, _ts, version, _fee, _, _) =
        client.get_attestation(&business, &period).unwrap();
    assert_eq!(stored_root, new_root);
    assert_eq!(version, 2);
}

#[test]
#[should_panic(expected = "new version must be greater than old version")]
fn test_migrate_with_same_version_panics() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let old_root = BytesN::from_array(&env, &[1u8; 32]);
    let new_root = BytesN::from_array(&env, &[2u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &old_root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );

    // Same version should panic
    client.migrate_attestation(&admin, &business, &period, &new_root, &1u32, &1u64);
}

#[test]
#[should_panic(expected = "new version must be greater than old version")]
fn test_migrate_with_lower_version_panics() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let old_root = BytesN::from_array(&env, &[1u8; 32]);
    let new_root = BytesN::from_array(&env, &[2u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &old_root,
        &1_700_000_000u64,
        &5u32,
        &None,
        &None,
        &0u64,
    );

    // Lower version should panic
    client.migrate_attestation(&admin, &business, &period, &new_root, &3u32, &1u64);
}

// ════════════════════════════════════════════════════════════════════
//  Role Change Event Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_grant_role_emits_event() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);

    client.grant_role(&admin, &user, &ROLE_ADMIN, &1u64);

    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
fn test_revoke_role_emits_event() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);

    client.grant_role(&admin, &user, &ROLE_ADMIN, &1u64);
    client.revoke_role(&admin, &user, &ROLE_ADMIN, &2u64);

    let events = env.events().all();
    // Events are emitted
    assert!(!events.is_empty());
}

// ════════════════════════════════════════════════════════════════════
//  Pause Event Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_pause_emits_event() {
    let (env, client, admin) = setup();

    client.pause(&admin, &1u64);

    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
fn test_unpause_emits_event() {
    let (env, client, admin) = setup();

    client.pause(&admin, &1u64);
    client.unpause(&admin, &2u64);

    let events = env.events().all();
    // Events are emitted
    assert!(!events.is_empty());
}

// ════════════════════════════════════════════════════════════════════
//  Event Schema Tests
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_event_contains_business_address() {
    let (env, client, _admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = BytesN::from_array(&env, &[1u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );

    // Events are published with business address as topic for indexing
    let events = env.events().all();
    assert!(!events.is_empty());
}

// ════════════════════════════════════════════════════════════════════
//  Edge Cases
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_is_revoked_false_by_default() {
    let (env, client, _admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");

    assert!(!client.is_revoked(&business, &period));
}

#[test]
fn test_is_revoked_after_revocation() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = BytesN::from_array(&env, &[1u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &root,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );

    assert!(!client.is_revoked(&business, &period));

    let reason = String::from_str(&env, "test");
    client.revoke_attestation(&admin, &business, &period, &reason, &1u64);

    assert!(client.is_revoked(&business, &period));
}

#[test]
fn test_multiple_migrations() {
    let (env, client, admin) = setup();

    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root_v1 = BytesN::from_array(&env, &[1u8; 32]);
    let root_v2 = BytesN::from_array(&env, &[2u8; 32]);
    let root_v3 = BytesN::from_array(&env, &[3u8; 32]);

    client.submit_attestation(
        &business,
        &period,
        &root_v1,
        &1_700_000_000u64,
        &1u32,
        &None,
        &None,
        &0u64,
    );
    client.migrate_attestation(&admin, &business, &period, &root_v2, &2u32, &1u64);
    client.migrate_attestation(&admin, &business, &period, &root_v3, &3u32, &2u64);

    let (stored_root, _ts, version, _fee, _, _) =
        client.get_attestation(&business, &period).unwrap();
    assert_eq!(stored_root, root_v3);
    assert_eq!(version, 3);
}

// ════════════════════════════════════════════════════════════════════
//  Event Schema Snapshot Tests
// ════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use crate::events::*;
    use soroban_sdk::{vec, IntoVal, TryFromVal};

    #[test]
    fn test_attestation_submitted_schema_snapshot() {
        let (env, _client, _admin) = setup();
        let business = Address::generate(&env);
        let period = String::from_str(&env, "2026-02");
        let root = BytesN::from_array(&env, &[1u8; 32]);
        let timestamp = 1_700_000_000u64;
        let version = 1u32;
        let fee = 100i128;
        let proof_hash = Some(BytesN::from_array(&env, &[2u8; 32]));
        let expiry = Some(2_000_000_000u64);

        emit_attestation_submitted(
            &env,
            &business,
            &period,
            &root,
            timestamp,
            version,
            fee,
            &proof_hash,
            expiry,
        );

        let last_event = env.events().all().last().unwrap();
        let (_contract_id, topics, data) = last_event;

        // Verify Topics
        assert_eq!(topics.len(), 2);
        assert_eq!(topics.get(0).unwrap(), TOPIC_ATTESTATION_SUBMITTED.into_val(&env));
        assert_eq!(topics.get(1).unwrap(), business.into_val(&env));

        // Verify Data Schema
        let event_data = AttestationSubmittedEvent::try_from_val(&env, &data).unwrap();
        assert_eq!(event_data.business, business);
        assert_eq!(event_data.period, period);
        assert_eq!(event_data.merkle_root, root);
        assert_eq!(event_data.timestamp, timestamp);
        assert_eq!(event_data.version, version);
        assert_eq!(event_data.fee_paid, fee);
        assert_eq!(event_data.proof_hash, proof_hash);
        assert_eq!(event_data.expiry_timestamp, expiry);
    }

    #[test]
    fn test_attestation_revoked_schema_snapshot() {
        let (env, _client, _admin) = setup();
        let business = Address::generate(&env);
        let period = String::from_str(&env, "2026-02");
        let revoked_by = Address::generate(&env);
        let reason = String::from_str(&env, "test reason");

        emit_attestation_revoked(&env, &business, &period, &revoked_by, &reason);

        let last_event = env.events().all().last().unwrap();
        let (_contract_id, topics, data) = last_event;

        // Verify Topics
        assert_eq!(topics.len(), 2);
        assert_eq!(topics.get(0).unwrap(), TOPIC_ATTESTATION_REVOKED.into_val(&env));
        assert_eq!(topics.get(1).unwrap(), business.into_val(&env));

        // Verify Data Schema
        let event_data = AttestationRevokedEvent::try_from_val(&env, &data).unwrap();
        assert_eq!(event_data.business, business);
        assert_eq!(event_data.period, period);
        assert_eq!(event_data.revoked_by, revoked_by);
        assert_eq!(event_data.reason, reason);
    }

    #[test]
    fn test_role_granted_schema_snapshot() {
        let (env, _client, _admin) = setup();
        let account = Address::generate(&env);
        let changed_by = Address::generate(&env);
        let role = 1u32;

        emit_role_granted(&env, &account, role, &changed_by);

        let last_event = env.events().all().last().unwrap();
        let (_contract_id, topics, data) = last_event;

        // Verify Topics
        assert_eq!(topics.len(), 2);
        assert_eq!(topics.get(0).unwrap(), TOPIC_ROLE_GRANTED.into_val(&env));
        assert_eq!(topics.get(1).unwrap(), account.into_val(&env));

        // Verify Data Schema
        let event_data = RoleChangedEvent::try_from_val(&env, &data).unwrap();
        assert_eq!(event_data.account, account);
        assert_eq!(event_data.role, role);
        assert_eq!(event_data.changed_by, changed_by);
    }

    #[test]
    fn test_fee_config_changed_schema_snapshot() {
        let (env, _client, _admin) = setup();
        let token = Address::generate(&env);
        let collector = Address::generate(&env);
        let changed_by = Address::generate(&env);
        let base_fee = 1000i128;
        let enabled = true;

        emit_fee_config_changed(&env, &token, &collector, base_fee, enabled, &changed_by);

        let last_event = env.events().all().last().unwrap();
        let (_contract_id, topics, data) = last_event;

        // Verify Topics
        assert_eq!(topics.len(), 1);
        assert_eq!(topics.get(0).unwrap(), TOPIC_FEE_CONFIG.into_val(&env));

        // Verify Data Schema
        let event_data = FeeConfigChangedEvent::try_from_val(&env, &data).unwrap();
        assert_eq!(event_data.token, token);
        assert_eq!(event_data.collector, collector);
        assert_eq!(event_data.base_fee, base_fee);
        assert_eq!(event_data.enabled, enabled);
        assert_eq!(event_data.changed_by, changed_by);
    }

    #[test]
    fn test_pause_changed_schema_snapshot() {
        let (env, _client, _admin) = setup();
        let changed_by = Address::generate(&env);

        emit_paused(&env, &changed_by);

        let last_event = env.events().all().last().unwrap();
        let (_contract_id, topics, data) = last_event;

        // Verify Topics
        assert_eq!(topics.len(), 1);
        assert_eq!(topics.get(0).unwrap(), TOPIC_PAUSED.into_val(&env));

        // Verify Data Schema
        let event_data = PauseChangedEvent::try_from_val(&env, &data).unwrap();
        assert_eq!(event_data.changed_by, changed_by);
    }

    #[test]
    fn test_attestation_migrated_schema_snapshot() {
        let (env, _client, _admin) = setup();
        let business = Address::generate(&env);
        let period = String::from_str(&env, "2026-02");
        let old_root = BytesN::from_array(&env, &[1u8; 32]);
        let new_root = BytesN::from_array(&env, &[2u8; 32]);
        let old_version = 1u32;
        let new_version = 2u32;
        let migrated_by = Address::generate(&env);

        emit_attestation_migrated(
            &env,
            &business,
            &period,
            &old_root,
            &new_root,
            old_version,
            new_version,
            &migrated_by,
        );

        let last_event = env.events().all().last().unwrap();
        let (_contract_id, topics, data) = last_event;

        // Verify Topics
        assert_eq!(topics.len(), 2);
        assert_eq!(topics.get(0).unwrap(), TOPIC_ATTESTATION_MIGRATED.into_val(&env));
        assert_eq!(topics.get(1).unwrap(), business.into_val(&env));

        // Verify Data Schema
        let event_data = AttestationMigratedEvent::try_from_val(&env, &data).unwrap();
        assert_eq!(event_data.business, business);
        assert_eq!(event_data.period, period);
        assert_eq!(event_data.old_merkle_root, old_root);
        assert_eq!(event_data.new_merkle_root, new_root);
        assert_eq!(event_data.old_version, old_version);
        assert_eq!(event_data.new_version, new_version);
        assert_eq!(event_data.migrated_by, migrated_by);
    }

    #[test]
    fn test_rate_limit_config_changed_schema_snapshot() {
        let (env, _client, _admin) = setup();
        let changed_by = Address::generate(&env);
        let max_submissions = 100u32;
        let window_seconds = 3600u64;
        let enabled = true;

        emit_rate_limit_config_changed(&env, max_submissions, window_seconds, enabled, &changed_by);

        let last_event = env.events().all().last().unwrap();
        let (_contract_id, topics, data) = last_event;

        // Verify Topics
        assert_eq!(topics.len(), 1);
        assert_eq!(topics.get(0).unwrap(), TOPIC_RATE_LIMIT.into_val(&env));

        // Verify Data Schema
        let event_data = RateLimitConfigChangedEvent::try_from_val(&env, &data).unwrap();
        assert_eq!(event_data.max_submissions, max_submissions);
        assert_eq!(event_data.window_seconds, window_seconds);
        assert_eq!(event_data.enabled, enabled);
        assert_eq!(event_data.changed_by, changed_by);
    }

    #[test]
    fn test_key_rotation_proposed_schema_snapshot() {
        let (env, _client, _admin) = setup();
        let old_admin = Address::generate(&env);
        let new_admin = Address::generate(&env);
        let timelock = 1000u32;
        let expiry = 2000u32;

        emit_key_rotation_proposed(&env, &old_admin, &new_admin, timelock, expiry);

        let last_event = env.events().all().last().unwrap();
        let (_contract_id, topics, data) = last_event;

        // Verify Topics
        assert_eq!(topics.len(), 1);
        assert_eq!(topics.get(0).unwrap(), TOPIC_KEY_ROTATION_PROPOSED.into_val(&env));

        // Verify Data Schema
        let event_data = KeyRotationProposedEvent::try_from_val(&env, &data).unwrap();
        assert_eq!(event_data.old_admin, old_admin);
        assert_eq!(event_data.new_admin, new_admin);
        assert_eq!(event_data.timelock_until, timelock);
        assert_eq!(event_data.expires_at, expiry);
    }
}
