//! Comprehensive tests for nonce-based replay protection with nonce partitioning.
//!
//! Coverage:
//!   - Core nonce semantics (start, increment, replay, skip, overflow)
//!   - Channel partitioning isolation (cross-channel, cross-actor)
//!   - Well-known channel constants and classification
//!   - Partition-aware bulk query/reset utilities
//!   - Adversarial cross-partition replay attack scenarios
//!   - Boundary values (u32::MAX channel, u64::MAX nonce, channel 0)
//!   - Multi-actor × multi-channel stress tests
//!   - Reset semantics and replay-after-reset
//!   - Ordering and determinism guarantees

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, Address, Env};

use crate::replay_protection::{
    get_nonce, get_nonces_for_channels, is_custom_channel, is_well_known_channel,
    peek_next_nonce, reset_nonce, reset_nonces_for_channels, verify_and_increment_nonce,
    CHANNEL_ADMIN, CHANNEL_BUSINESS, CHANNEL_CUSTOM_START, CHANNEL_GOVERNANCE, CHANNEL_MULTISIG,
    CHANNEL_PROTOCOL,
};

#[contract]
pub struct ReplayProtectionTestContract;

#[contractimpl]
impl ReplayProtectionTestContract {
    pub fn test_function(_env: Env) -> u32 {
        // Simple function to satisfy contract requirement
        42
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  Core Nonce Semantics
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn nonce_starts_at_zero_and_increments() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = 1u32;

    env.as_contract(&contract_id, || {
        // Fresh pair starts at 0.
        assert_eq!(get_nonce(&env, &actor, channel), 0);
        assert_eq!(peek_next_nonce(&env, &actor, channel), 0);

        // First valid call uses nonce = 0.
        verify_and_increment_nonce(&env, &actor, channel, 0);
        assert_eq!(get_nonce(&env, &actor, channel), 1);

        // Next call uses nonce = 1.
        verify_and_increment_nonce(&env, &actor, channel, 1);
        assert_eq!(get_nonce(&env, &actor, channel), 2);
    });
}

#[test]
#[should_panic(expected = "nonce mismatch")]
fn replay_with_same_nonce_panics() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = 2u32;

    env.as_contract(&contract_id, || {
        // First call with 0 succeeds.
        verify_and_increment_nonce(&env, &actor, channel, 0);

        // Replaying 0 again must panic.
        verify_and_increment_nonce(&env, &actor, channel, 0);
    });
}

#[test]
#[should_panic(expected = "nonce mismatch")]
fn skipped_nonce_panics() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = 3u32;

    env.as_contract(&contract_id, || {
        // Current is implicitly 0; trying to jump to 1 should fail.
        verify_and_increment_nonce(&env, &actor, channel, 1);
    });
}

#[test]
fn different_actors_have_independent_nonces() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor_a = Address::generate(&env);
    let actor_b = Address::generate(&env);
    let channel = 4u32;

    env.as_contract(&contract_id, || {
        // Each actor starts at 0.
        assert_eq!(get_nonce(&env, &actor_a, channel), 0);
        assert_eq!(get_nonce(&env, &actor_b, channel), 0);

        // Increment actor A twice.
        verify_and_increment_nonce(&env, &actor_a, channel, 0);
        verify_and_increment_nonce(&env, &actor_a, channel, 1);

        // Actor B is unaffected.
        assert_eq!(get_nonce(&env, &actor_b, channel), 0);
    });
}

#[test]
fn different_channels_have_independent_nonces() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel_admin = 10u32;
    let channel_business = 11u32;

    env.as_contract(&contract_id, || {
        // Both channels start at 0 for the same actor.
        assert_eq!(get_nonce(&env, &actor, channel_admin), 0);
        assert_eq!(get_nonce(&env, &actor, channel_business), 0);

        // Use admin channel twice.
        verify_and_increment_nonce(&env, &actor, channel_admin, 0);
        verify_and_increment_nonce(&env, &actor, channel_admin, 1);

        // Business channel is still untouched.
        assert_eq!(get_nonce(&env, &actor, channel_business), 0);
    });
}

#[test]
#[should_panic(expected = "nonce overflow")]
fn overflow_panics() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = 99u32;

    env.as_contract(&contract_id, || {
        // Manually set the nonce near the maximum to force overflow behaviour.
        use crate::replay_protection::ReplayKey;
        env.storage()
            .instance()
            .set(&ReplayKey::Nonce(actor.clone(), channel), &u64::MAX);

        // Any attempt to use u64::MAX should panic on overflow check.
        verify_and_increment_nonce(&env, &actor, channel, u64::MAX);
    });
}

#[test]
fn concurrent_actors_same_channel() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor_a = Address::generate(&env);
    let actor_b = Address::generate(&env);
    let actor_c = Address::generate(&env);
    let channel = 42u32;

    env.as_contract(&contract_id, || {
        // All actors start at 0
        assert_eq!(get_nonce(&env, &actor_a, channel), 0);
        assert_eq!(get_nonce(&env, &actor_b, channel), 0);
        assert_eq!(get_nonce(&env, &actor_c, channel), 0);

        // Actor A advances to nonce 3
        verify_and_increment_nonce(&env, &actor_a, channel, 0);
        verify_and_increment_nonce(&env, &actor_a, channel, 1);
        verify_and_increment_nonce(&env, &actor_a, channel, 2);
        assert_eq!(get_nonce(&env, &actor_a, channel), 3);

        // Actor B advances to nonce 1
        verify_and_increment_nonce(&env, &actor_b, channel, 0);
        assert_eq!(get_nonce(&env, &actor_b, channel), 1);

        // Actor C is still at 0
        assert_eq!(get_nonce(&env, &actor_c, channel), 0);

        // Each actor can only use their current nonce
        verify_and_increment_nonce(&env, &actor_a, channel, 3); // Works
        verify_and_increment_nonce(&env, &actor_b, channel, 1); // Works
        verify_and_increment_nonce(&env, &actor_c, channel, 0); // Works

        // Final state
        assert_eq!(get_nonce(&env, &actor_a, channel), 4);
        assert_eq!(get_nonce(&env, &actor_b, channel), 2);
        assert_eq!(get_nonce(&env, &actor_c, channel), 1);
    });
}

#[test]
fn peek_next_nonce_consistency() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = 100u32;

    env.as_contract(&contract_id, || {
        // Initially both should return 0
        assert_eq!(get_nonce(&env, &actor, channel), 0);
        assert_eq!(peek_next_nonce(&env, &actor, channel), 0);

        // After incrementing, both should return 1
        verify_and_increment_nonce(&env, &actor, channel, 0);
        assert_eq!(get_nonce(&env, &actor, channel), 1);
        assert_eq!(peek_next_nonce(&env, &actor, channel), 1);

        // After multiple increments
        verify_and_increment_nonce(&env, &actor, channel, 1);
        verify_and_increment_nonce(&env, &actor, channel, 2);
        assert_eq!(get_nonce(&env, &actor, channel), 3);
        assert_eq!(peek_next_nonce(&env, &actor, channel), 3);
    });
}

#[test]
#[should_panic(expected = "nonce mismatch")]
fn negative_nonce_rejected() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = 200u32;

    env.as_contract(&contract_id, || {
        // Advance to nonce 5
        for i in 0..5 {
            verify_and_increment_nonce(&env, &actor, channel, i);
        }
        assert_eq!(get_nonce(&env, &actor, channel), 5);

        // Try to go backwards - should panic
        verify_and_increment_nonce(&env, &actor, channel, 3);
    });
}

#[test]
#[should_panic(expected = "nonce mismatch")]
fn double_increment_same_nonce_panics() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = 300u32;

    env.as_contract(&contract_id, || {
        // Use nonce 0 successfully
        verify_and_increment_nonce(&env, &actor, channel, 0);
        assert_eq!(get_nonce(&env, &actor, channel), 1);

        // Try to use nonce 0 again - should panic
        verify_and_increment_nonce(&env, &actor, channel, 0);
    });
}

#[test]
fn multi_channel_independence_stress_test() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channels = [1u32, 10u32, 100u32, 999u32, u32::MAX];

    env.as_contract(&contract_id, || {
        // Each channel should start at 0
        for &channel in &channels {
            assert_eq!(get_nonce(&env, &actor, channel), 0);
        }

        // Advance each channel to different nonce values
        for (i, &channel) in channels.iter().enumerate() {
            for j in 0..=i {
                verify_and_increment_nonce(&env, &actor, channel, j as u64);
            }
        }

        // Verify final states
        for (i, &channel) in channels.iter().enumerate() {
            assert_eq!(get_nonce(&env, &actor, channel), (i + 1) as u64);
        }
    });
}

#[test]
fn large_nonce_values() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = 999u32;

    env.as_contract(&contract_id, || {
        // Manually set a large nonce value
        use crate::replay_protection::ReplayKey;
        let large_nonce = u64::MAX - 10;
        env.storage()
            .instance()
            .set(&ReplayKey::Nonce(actor.clone(), channel), &large_nonce);

        // Should be able to use the large nonce
        assert_eq!(get_nonce(&env, &actor, channel), large_nonce);
        verify_and_increment_nonce(&env, &actor, channel, large_nonce);
        assert_eq!(get_nonce(&env, &actor, channel), large_nonce + 1);
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Well-Known Channel Constants
// ════════════════════════════════════════════════════════════════════════════

/// Verify that well-known channel constants have the documented values.
#[test]
fn well_known_channel_values() {
    assert_eq!(CHANNEL_ADMIN, 1);
    assert_eq!(CHANNEL_BUSINESS, 2);
    assert_eq!(CHANNEL_MULTISIG, 3);
    assert_eq!(CHANNEL_GOVERNANCE, 4);
    assert_eq!(CHANNEL_PROTOCOL, 5);
    assert_eq!(CHANNEL_CUSTOM_START, 256);
}

/// Verify that all well-known channels are distinct.
#[test]
fn well_known_channels_are_distinct() {
    let channels = [
        CHANNEL_ADMIN,
        CHANNEL_BUSINESS,
        CHANNEL_MULTISIG,
        CHANNEL_GOVERNANCE,
        CHANNEL_PROTOCOL,
    ];
    for i in 0..channels.len() {
        for j in (i + 1)..channels.len() {
            assert_ne!(
                channels[i], channels[j],
                "Channel {} and {} must be distinct",
                i, j
            );
        }
    }
}

/// Verify that well-known channels are below CHANNEL_CUSTOM_START.
#[test]
fn well_known_channels_below_custom_start() {
    assert!(CHANNEL_ADMIN < CHANNEL_CUSTOM_START);
    assert!(CHANNEL_BUSINESS < CHANNEL_CUSTOM_START);
    assert!(CHANNEL_MULTISIG < CHANNEL_CUSTOM_START);
    assert!(CHANNEL_GOVERNANCE < CHANNEL_CUSTOM_START);
    assert!(CHANNEL_PROTOCOL < CHANNEL_CUSTOM_START);
}

// ════════════════════════════════════════════════════════════════════════════
//  Channel Classification Helpers
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn is_well_known_channel_classification() {
    // Well-known channels return true
    assert!(is_well_known_channel(CHANNEL_ADMIN));
    assert!(is_well_known_channel(CHANNEL_BUSINESS));
    assert!(is_well_known_channel(CHANNEL_MULTISIG));
    assert!(is_well_known_channel(CHANNEL_GOVERNANCE));
    assert!(is_well_known_channel(CHANNEL_PROTOCOL));

    // Channel 0 is NOT well-known
    assert!(!is_well_known_channel(0));

    // Values above CHANNEL_PROTOCOL are not well-known
    assert!(!is_well_known_channel(6));
    assert!(!is_well_known_channel(CHANNEL_CUSTOM_START));
    assert!(!is_well_known_channel(u32::MAX));
}

#[test]
fn is_custom_channel_classification() {
    // Values below CHANNEL_CUSTOM_START are not custom
    assert!(!is_custom_channel(0));
    assert!(!is_custom_channel(CHANNEL_ADMIN));
    assert!(!is_custom_channel(CHANNEL_PROTOCOL));
    assert!(!is_custom_channel(CHANNEL_CUSTOM_START - 1));

    // CHANNEL_CUSTOM_START and above are custom
    assert!(is_custom_channel(CHANNEL_CUSTOM_START));
    assert!(is_custom_channel(CHANNEL_CUSTOM_START + 1));
    assert!(is_custom_channel(1000));
    assert!(is_custom_channel(u32::MAX));
}

// ════════════════════════════════════════════════════════════════════════════
//  Nonce Partitioning – Cross-Channel Isolation
// ════════════════════════════════════════════════════════════════════════════

/// The fundamental partitioning invariant: advancing the nonce on one
/// well-known channel must not affect ANY other channel for the same actor.
#[test]
fn partitioning_well_known_channels_are_fully_isolated() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let all_channels = [
            CHANNEL_ADMIN,
            CHANNEL_BUSINESS,
            CHANNEL_MULTISIG,
            CHANNEL_GOVERNANCE,
            CHANNEL_PROTOCOL,
        ];

        // All start at 0
        for &ch in &all_channels {
            assert_eq!(get_nonce(&env, &actor, ch), 0);
        }

        // Advance ADMIN to 5
        for i in 0..5u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, i);
        }
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 5);

        // All other channels must still be 0
        assert_eq!(get_nonce(&env, &actor, CHANNEL_BUSINESS), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_MULTISIG), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_GOVERNANCE), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_PROTOCOL), 0);

        // Advance BUSINESS to 3
        for i in 0..3u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_BUSINESS, i);
        }

        // ADMIN still at 5, BUSINESS at 3, rest at 0
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 5);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_BUSINESS), 3);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_MULTISIG), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_GOVERNANCE), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_PROTOCOL), 0);
    });
}

/// Custom channels are also fully isolated from well-known channels.
#[test]
fn partitioning_custom_channel_isolated_from_well_known() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let custom_channel = CHANNEL_CUSTOM_START + 42;

    env.as_contract(&contract_id, || {
        // Advance custom channel
        verify_and_increment_nonce(&env, &actor, custom_channel, 0);
        verify_and_increment_nonce(&env, &actor, custom_channel, 1);
        assert_eq!(get_nonce(&env, &actor, custom_channel), 2);

        // Well-known channels unaffected
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_BUSINESS), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_MULTISIG), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_GOVERNANCE), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_PROTOCOL), 0);
    });
}

/// Two different custom channels for the same actor are isolated.
#[test]
fn partitioning_multiple_custom_channels_isolated() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let ch_a = CHANNEL_CUSTOM_START;
    let ch_b = CHANNEL_CUSTOM_START + 1;
    let ch_c = CHANNEL_CUSTOM_START + 999;

    env.as_contract(&contract_id, || {
        verify_and_increment_nonce(&env, &actor, ch_a, 0);
        verify_and_increment_nonce(&env, &actor, ch_a, 1);
        verify_and_increment_nonce(&env, &actor, ch_a, 2);

        verify_and_increment_nonce(&env, &actor, ch_b, 0);

        assert_eq!(get_nonce(&env, &actor, ch_a), 3);
        assert_eq!(get_nonce(&env, &actor, ch_b), 1);
        assert_eq!(get_nonce(&env, &actor, ch_c), 0);
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Nonce Partitioning – Cross-Actor Isolation
// ════════════════════════════════════════════════════════════════════════════

/// Different actors using the same well-known channel are fully independent.
#[test]
fn partitioning_different_actors_same_well_known_channel() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let admin = Address::generate(&env);
    let business = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Admin uses CHANNEL_ADMIN
        verify_and_increment_nonce(&env, &admin, CHANNEL_ADMIN, 0);
        verify_and_increment_nonce(&env, &admin, CHANNEL_ADMIN, 1);
        verify_and_increment_nonce(&env, &admin, CHANNEL_ADMIN, 2);

        // Business uses same CHANNEL_ADMIN (different actor)
        verify_and_increment_nonce(&env, &business, CHANNEL_ADMIN, 0);

        assert_eq!(get_nonce(&env, &admin, CHANNEL_ADMIN), 3);
        assert_eq!(get_nonce(&env, &business, CHANNEL_ADMIN), 1);
    });
}

/// Each actor × channel combination is a completely independent nonce stream.
#[test]
fn partitioning_actor_channel_cartesian_product_independent() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actors: [Address; 3] = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    let channels = [CHANNEL_ADMIN, CHANNEL_BUSINESS, CHANNEL_MULTISIG];

    env.as_contract(&contract_id, || {
        // Advance each (actor, channel) pair to a unique nonce
        for (ai, actor) in actors.iter().enumerate() {
            for (ci, &channel) in channels.iter().enumerate() {
                let target = (ai * 3 + ci) as u64;
                for n in 0..target {
                    verify_and_increment_nonce(&env, actor, channel, n);
                }
            }
        }

        // Verify each pair has the expected nonce
        for (ai, actor) in actors.iter().enumerate() {
            for (ci, &channel) in channels.iter().enumerate() {
                let expected = (ai * 3 + ci) as u64;
                assert_eq!(
                    get_nonce(&env, actor, channel),
                    expected,
                    "Actor {} channel {} should be at nonce {}",
                    ai,
                    ci,
                    expected
                );
            }
        }
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Adversarial Cross-Partition Replay Attacks
// ════════════════════════════════════════════════════════════════════════════

/// An admin nonce value cannot be replayed on the business channel.
/// Even if admin nonce is currently at 5 and business nonce is at 0,
/// providing nonce=5 on the business channel must fail.
#[test]
#[should_panic(expected = "nonce mismatch")]
fn adversarial_admin_nonce_replayed_on_business_channel() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Advance admin channel to 5
        for i in 0..5u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, i);
        }
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 5);

        // Business channel is at 0. Trying to use nonce 5 should fail.
        verify_and_increment_nonce(&env, &actor, CHANNEL_BUSINESS, 5);
    });
}

/// A business nonce value cannot be replayed on the governance channel.
#[test]
#[should_panic(expected = "nonce mismatch")]
fn adversarial_business_nonce_replayed_on_governance_channel() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Advance business channel to 3
        for i in 0..3u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_BUSINESS, i);
        }

        // Governance is at 0. Trying nonce 3 should fail.
        verify_and_increment_nonce(&env, &actor, CHANNEL_GOVERNANCE, 3);
    });
}

/// An actor's nonce on one channel cannot be used by a different actor
/// on the same channel (cross-actor attack).
#[test]
#[should_panic(expected = "nonce mismatch")]
fn adversarial_cross_actor_nonce_replay() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let legit_actor = Address::generate(&env);
    let attacker = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Legit actor advances to nonce 3
        for i in 0..3u64 {
            verify_and_increment_nonce(&env, &legit_actor, CHANNEL_ADMIN, i);
        }

        // Attacker tries to use nonce 3 (their current nonce is 0)
        verify_and_increment_nonce(&env, &attacker, CHANNEL_ADMIN, 3);
    });
}

/// Rapid replay attempt: consuming the same nonce twice in the same
/// "transaction" (sequential calls) must fail on the second attempt.
#[test]
#[should_panic(expected = "nonce mismatch")]
fn adversarial_rapid_double_spend() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
        // Immediately try to replay nonce 0
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
    });
}

/// Attempting to skip a nonce on a channel that has a nonce on a different channel
/// at that value must still fail (cross-partition nonce confusion attack).
#[test]
#[should_panic(expected = "nonce mismatch")]
fn adversarial_nonce_confusion_skip_via_other_channel() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Admin is at 0, advance multisig to 5
        for i in 0..5u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_MULTISIG, i);
        }

        // Admin is still at 0. Trying to skip to 5 (influenced by multisig's state)
        // must fail.
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 5);
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Bulk Query: get_nonces_for_channels
// ════════════════════════════════════════════════════════════════════════════

/// Bulk query returns correct nonces for all specified channels.
#[test]
fn bulk_query_returns_correct_nonces() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Advance channels to different nonce values
        // ADMIN -> 3
        for i in 0..3u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, i);
        }
        // BUSINESS -> 1
        verify_and_increment_nonce(&env, &actor, CHANNEL_BUSINESS, 0);
        // GOVERNANCE -> 0 (untouched)

        let channels = [CHANNEL_ADMIN, CHANNEL_BUSINESS, CHANNEL_GOVERNANCE];
        let nonces = get_nonces_for_channels(&env, &actor, &channels);

        assert_eq!(nonces.len(), 3);
        assert_eq!(nonces.get(0).unwrap(), 3u64);
        assert_eq!(nonces.get(1).unwrap(), 1u64);
        assert_eq!(nonces.get(2).unwrap(), 0u64);
    });
}

/// Bulk query with empty channel list returns empty vec.
#[test]
fn bulk_query_empty_channels() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let nonces = get_nonces_for_channels(&env, &actor, &[]);
        assert_eq!(nonces.len(), 0);
    });
}

/// Bulk query with a single channel behaves like get_nonce.
#[test]
fn bulk_query_single_channel() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
        let nonces = get_nonces_for_channels(&env, &actor, &[CHANNEL_ADMIN]);
        assert_eq!(nonces.len(), 1);
        assert_eq!(nonces.get(0).unwrap(), 1u64);
    });
}

/// Bulk query all well-known channels for a fresh actor returns all zeros.
#[test]
fn bulk_query_all_well_known_fresh_actor() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let channels = [
            CHANNEL_ADMIN,
            CHANNEL_BUSINESS,
            CHANNEL_MULTISIG,
            CHANNEL_GOVERNANCE,
            CHANNEL_PROTOCOL,
        ];
        let nonces = get_nonces_for_channels(&env, &actor, &channels);
        assert_eq!(nonces.len(), 5);
        for i in 0..5 {
            assert_eq!(nonces.get(i).unwrap(), 0u64);
        }
    });
}

/// Bulk query preserves ordering: results match the order of input channels.
#[test]
fn bulk_query_preserves_ordering() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Set distinct nonces: ADMIN=2, BUSINESS=0, PROTOCOL=7
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 1);
        for i in 0..7u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_PROTOCOL, i);
        }

        // Query in reverse order
        let nonces =
            get_nonces_for_channels(&env, &actor, &[CHANNEL_PROTOCOL, CHANNEL_BUSINESS, CHANNEL_ADMIN]);
        assert_eq!(nonces.get(0).unwrap(), 7u64); // PROTOCOL
        assert_eq!(nonces.get(1).unwrap(), 0u64); // BUSINESS
        assert_eq!(nonces.get(2).unwrap(), 2u64); // ADMIN
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Reset Nonce Utilities
// ════════════════════════════════════════════════════════════════════════════

/// Resetting a nonce returns it to 0 and allows nonce 0 to be used again.
#[test]
fn reset_nonce_returns_to_zero() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Advance to nonce 5
        for i in 0..5u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, i);
        }
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 5);

        // Reset
        reset_nonce(&env, &actor, CHANNEL_ADMIN);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 0);

        // Nonce 0 is valid again after reset
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 1);
    });
}

/// Resetting a nonce on one channel does not affect other channels.
#[test]
fn reset_nonce_does_not_affect_other_channels() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Advance admin and business
        for i in 0..3u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, i);
        }
        for i in 0..2u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_BUSINESS, i);
        }

        // Reset only admin
        reset_nonce(&env, &actor, CHANNEL_ADMIN);

        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 0);
        assert_eq!(
            get_nonce(&env, &actor, CHANNEL_BUSINESS),
            2,
            "Business channel must be unaffected by admin reset"
        );
    });
}

/// Resetting a nonce that was never incremented (still 0) is a no-op.
#[test]
fn reset_nonce_on_fresh_channel_is_noop() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 0);
        reset_nonce(&env, &actor, CHANNEL_ADMIN);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 0);

        // Normal increment still works
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 1);
    });
}

/// Bulk reset across multiple channels.
#[test]
fn reset_nonces_for_channels_bulk() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Advance several channels
        for i in 0..4u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, i);
        }
        for i in 0..2u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_BUSINESS, i);
        }
        verify_and_increment_nonce(&env, &actor, CHANNEL_GOVERNANCE, 0);

        // Bulk reset admin and business (leave governance)
        reset_nonces_for_channels(&env, &actor, &[CHANNEL_ADMIN, CHANNEL_BUSINESS]);

        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_BUSINESS), 0);
        assert_eq!(
            get_nonce(&env, &actor, CHANNEL_GOVERNANCE),
            1,
            "Governance must be unaffected by bulk reset"
        );
    });
}

/// Bulk reset with empty channel list is a no-op.
#[test]
fn reset_nonces_for_channels_empty_is_noop() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
        reset_nonces_for_channels(&env, &actor, &[]);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 1, "Nothing should change");
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Boundary Values
// ════════════════════════════════════════════════════════════════════════════

/// Channel 0 is valid and independent of well-known channels.
#[test]
fn channel_zero_works_and_is_independent() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        verify_and_increment_nonce(&env, &actor, 0u32, 0);
        assert_eq!(get_nonce(&env, &actor, 0u32), 1);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 0);
    });
}

/// u32::MAX channel is valid and usable.
#[test]
fn channel_u32_max_works() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        verify_and_increment_nonce(&env, &actor, u32::MAX, 0);
        verify_and_increment_nonce(&env, &actor, u32::MAX, 1);
        assert_eq!(get_nonce(&env, &actor, u32::MAX), 2);
    });
}

/// Nonce at u64::MAX - 1 can be used exactly once, then u64::MAX overflows.
#[test]
fn nonce_at_penultimate_value_succeeds_then_overflows() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = CHANNEL_ADMIN;

    env.as_contract(&contract_id, || {
        use crate::replay_protection::ReplayKey;
        let penultimate = u64::MAX - 1;
        env.storage()
            .instance()
            .set(&ReplayKey::Nonce(actor.clone(), channel), &penultimate);

        // This should succeed: penultimate -> MAX
        verify_and_increment_nonce(&env, &actor, channel, penultimate);
        assert_eq!(get_nonce(&env, &actor, channel), u64::MAX);
    });
}

/// After reaching u64::MAX, the next call must overflow-panic.
#[test]
#[should_panic(expected = "nonce overflow")]
fn nonce_overflow_at_u64_max() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);
    let channel = CHANNEL_BUSINESS;

    env.as_contract(&contract_id, || {
        use crate::replay_protection::ReplayKey;
        env.storage()
            .instance()
            .set(&ReplayKey::Nonce(actor.clone(), channel), &u64::MAX);

        verify_and_increment_nonce(&env, &actor, channel, u64::MAX);
    });
}

/// Adjacent channel ids are independent (no off-by-one in storage keys).
#[test]
fn adjacent_channel_ids_are_independent() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Use channels 254, 255, 256
        verify_and_increment_nonce(&env, &actor, 254, 0);
        verify_and_increment_nonce(&env, &actor, 254, 1);
        verify_and_increment_nonce(&env, &actor, 255, 0);
        // 256 is CHANNEL_CUSTOM_START, untouched

        assert_eq!(get_nonce(&env, &actor, 254), 2);
        assert_eq!(get_nonce(&env, &actor, 255), 1);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_CUSTOM_START), 0);
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Multi-Actor × Multi-Channel Stress Tests
// ════════════════════════════════════════════════════════════════════════════

/// 5 actors × 5 well-known channels = 25 independent nonce streams.
/// Each stream is advanced to a unique value and verified.
#[test]
fn stress_five_actors_five_channels() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actors: Vec<Address> = (0..5).map(|_| Address::generate(&env)).collect();
    let channels = [
        CHANNEL_ADMIN,
        CHANNEL_BUSINESS,
        CHANNEL_MULTISIG,
        CHANNEL_GOVERNANCE,
        CHANNEL_PROTOCOL,
    ];

    env.as_contract(&contract_id, || {
        // Each (actor_idx, channel_idx) pair gets a unique target nonce
        for (ai, actor) in actors.iter().enumerate() {
            for (ci, &channel) in channels.iter().enumerate() {
                let target = ((ai + 1) * (ci + 1)) as u64;
                for n in 0..target {
                    verify_and_increment_nonce(&env, actor, channel, n);
                }
            }
        }

        // Verify all 25 streams
        for (ai, actor) in actors.iter().enumerate() {
            for (ci, &channel) in channels.iter().enumerate() {
                let expected = ((ai + 1) * (ci + 1)) as u64;
                assert_eq!(
                    get_nonce(&env, actor, channel),
                    expected,
                    "Actor {} channel {} should be at {}",
                    ai,
                    ci,
                    expected
                );
            }
        }
    });
}

/// Interleaved operations across actors and channels maintain isolation.
#[test]
fn stress_interleaved_operations() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor_a = Address::generate(&env);
    let actor_b = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Interleave operations across actors and channels
        verify_and_increment_nonce(&env, &actor_a, CHANNEL_ADMIN, 0);
        verify_and_increment_nonce(&env, &actor_b, CHANNEL_BUSINESS, 0);
        verify_and_increment_nonce(&env, &actor_a, CHANNEL_BUSINESS, 0);
        verify_and_increment_nonce(&env, &actor_b, CHANNEL_ADMIN, 0);
        verify_and_increment_nonce(&env, &actor_a, CHANNEL_ADMIN, 1);
        verify_and_increment_nonce(&env, &actor_a, CHANNEL_MULTISIG, 0);
        verify_and_increment_nonce(&env, &actor_b, CHANNEL_BUSINESS, 1);
        verify_and_increment_nonce(&env, &actor_b, CHANNEL_ADMIN, 1);

        assert_eq!(get_nonce(&env, &actor_a, CHANNEL_ADMIN), 2);
        assert_eq!(get_nonce(&env, &actor_a, CHANNEL_BUSINESS), 1);
        assert_eq!(get_nonce(&env, &actor_a, CHANNEL_MULTISIG), 1);
        assert_eq!(get_nonce(&env, &actor_b, CHANNEL_ADMIN), 2);
        assert_eq!(get_nonce(&env, &actor_b, CHANNEL_BUSINESS), 2);
        assert_eq!(get_nonce(&env, &actor_b, CHANNEL_MULTISIG), 0);
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Reset + Replay-After-Reset Scenarios
// ════════════════════════════════════════════════════════════════════════════

/// After reset, previously-used nonces (0, 1, 2, ...) become valid again
/// in sequential order.
#[test]
fn reset_allows_full_nonce_sequence_replay() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // First lifecycle: 0, 1, 2, 3, 4
        for i in 0..5u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, i);
        }
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 5);

        // Reset
        reset_nonce(&env, &actor, CHANNEL_ADMIN);

        // Second lifecycle: 0, 1, 2 works normally
        for i in 0..3u64 {
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, i);
        }
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 3);
    });
}

/// Multiple resets: each reset starts a fresh lifecycle.
#[test]
fn multiple_resets_each_start_fresh() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        for _round in 0..3 {
            // Each round: use 0 and 1, then reset
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 1);
            assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 2);
            reset_nonce(&env, &actor, CHANNEL_ADMIN);
            assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 0);
        }

        // Final: still starts at 0
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 1);
    });
}

/// Reset one actor does not affect another actor on the same channel.
#[test]
fn reset_one_actor_does_not_affect_other_actor() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor_a = Address::generate(&env);
    let actor_b = Address::generate(&env);

    env.as_contract(&contract_id, || {
        for i in 0..3u64 {
            verify_and_increment_nonce(&env, &actor_a, CHANNEL_ADMIN, i);
            verify_and_increment_nonce(&env, &actor_b, CHANNEL_ADMIN, i);
        }

        // Reset only actor_a
        reset_nonce(&env, &actor_a, CHANNEL_ADMIN);

        assert_eq!(get_nonce(&env, &actor_a, CHANNEL_ADMIN), 0);
        assert_eq!(
            get_nonce(&env, &actor_b, CHANNEL_ADMIN),
            3,
            "actor_b must be unaffected"
        );
    });
}

// ════════════════════════════════════════════════════════════════════════════
//  Determinism and Ordering Guarantees
// ════════════════════════════════════════════════════════════════════════════

/// get_nonce is idempotent: calling it multiple times without increment
/// always returns the same value.
#[test]
fn get_nonce_is_idempotent() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, 0);

        for _ in 0..10 {
            assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 1);
        }
    });
}

/// Sequential increments produce a strict +1 monotonic sequence with
/// no gaps, regardless of other channels' activity.
#[test]
fn strict_monotonic_increment_sequence() {
    let env = Env::default();
    let contract_id = env.register(ReplayProtectionTestContract, ());
    let actor = Address::generate(&env);

    env.as_contract(&contract_id, || {
        for expected in 0..20u64 {
            assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), expected);
            verify_and_increment_nonce(&env, &actor, CHANNEL_ADMIN, expected);
            assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), expected + 1);

            // Interleave noise on another channel
            if expected % 3 == 0 {
                verify_and_increment_nonce(
                    &env,
                    &actor,
                    CHANNEL_BUSINESS,
                    expected / 3,
                );
            }
        }
        assert_eq!(get_nonce(&env, &actor, CHANNEL_ADMIN), 20);
        assert_eq!(get_nonce(&env, &actor, CHANNEL_BUSINESS), 7); // 0,3,6,9,12,15,18 = 7 increments
    });
}
