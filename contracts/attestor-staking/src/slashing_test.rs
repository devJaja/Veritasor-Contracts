use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger as _}, token, Address, Env};
use soroban_sdk::{contract, contractimpl};
use proptest::prelude::*;

#[contract]
struct DummyDisputeContract;

#[contractimpl]
impl DummyDisputeContract {}

fn create_token_contract<'a>(
    env: &Env,
    admin: &Address,
) -> (Address, token::StellarAssetClient<'a>, token::Client<'a>) {
    let contract_id = env.register_stellar_asset_contract_v2(admin.clone());
    let addr = contract_id.address();
    (
        addr.clone(),
        token::StellarAssetClient::new(env, &addr),
        token::Client::new(env, &addr),
    )
}

/// Shared setup helper: initializes the contract and stakes `stake_amount` tokens for the attestor.
/// Returns `(attestor, treasury, dispute_contract, token_id, client)`.
fn setup(
    env: &Env,
    stake_amount: i128,
) -> (
    Address,
    Address,
    Address,
    Address,
    AttestorStakingContractClient<'_>,
) {
    let admin = Address::generate(env);
    let attestor = Address::generate(env);
    let treasury = Address::generate(env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, _token_client) = create_token_contract(env, &admin);
    token_admin.mint(&attestor, &stake_amount);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(env, &contract_id);

    client.initialize(&admin, &token_id, &treasury, &1000, &dispute_contract, &0u64);
    client.stake(&attestor, &stake_amount);

    (attestor, treasury, dispute_contract, token_id, client)
}

#[test]
fn test_slash_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &token_id,
        &treasury,
        &1000,
        &dispute_contract,
        &0u64,
    );
    client.stake(&attestor, &5000);

    let initial_treasury_balance = token_client.balance(&treasury);

    // Slash 2000 tokens
    env.as_contract(&dispute_contract, || {
        let outcome = client.slash(&attestor, &2000, &1);
        assert_eq!(outcome, SlashOutcome::Slashed);
    });

    let stake = client.get_stake(&attestor).unwrap();
    assert_eq!(stake.amount, 3000);

    let treasury_balance = token_client.balance(&treasury);
    assert_eq!(treasury_balance, initial_treasury_balance + 2000);
}

#[test]
fn test_slash_partial_when_insufficient_stake() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &token_id,
        &treasury,
        &1000,
        &dispute_contract,
        &0u64,
    );
    client.stake(&attestor, &2000);

    let initial_treasury_balance = token_client.balance(&treasury);

    // Try to slash 5000 but only 2000 available
    env.as_contract(&dispute_contract, || {
        let outcome = client.slash(&attestor, &5000, &1);
        assert_eq!(outcome, SlashOutcome::Slashed);
    });

    let stake = client.get_stake(&attestor).unwrap();
    assert_eq!(stake.amount, 0);

    let treasury_balance = token_client.balance(&treasury);
    assert_eq!(treasury_balance, initial_treasury_balance + 2000);
}

#[test]
#[should_panic(expected = "dispute already processed")]
fn test_slash_double_slashing_prevented() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, _token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &token_id,
        &treasury,
        &1000,
        &dispute_contract,
        &0u64,
    );
    client.stake(&attestor, &5000);

    env.as_contract(&dispute_contract, || {
        client.slash(&attestor, &2000, &1);
    });
    // Second slash with same dispute_id should panic
    env.as_contract(&dispute_contract, || {
        client.slash(&attestor, &1000, &1);
    });
}

#[test]
fn test_slash_multiple_disputes() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &token_id,
        &treasury,
        &1000,
        &dispute_contract,
        &0u64,
    );
    client.stake(&attestor, &5000);

    let initial_treasury_balance = token_client.balance(&treasury);

    // Slash for dispute 1
    env.as_contract(&dispute_contract, || {
        client.slash(&attestor, &1000, &1);
    });
    // Slash for dispute 2 (different dispute_id)
    env.as_contract(&dispute_contract, || {
        client.slash(&attestor, &1500, &2);
    });

    let stake = client.get_stake(&attestor).unwrap();
    assert_eq!(stake.amount, 2500);

    let treasury_balance = token_client.balance(&treasury);
    assert_eq!(treasury_balance, initial_treasury_balance + 2500);
}

#[test]
fn test_slash_no_stake() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, _token_admin, _token_client) = create_token_contract(&env, &admin);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &token_id,
        &treasury,
        &1000,
        &dispute_contract,
        &0u64,
    );

    // Try to slash attestor with no stake - should panic
    let result = client.try_slash(&attestor, &1000, &1);
    assert!(result.is_err());
}

#[test]
fn test_slash_zero_stake_returns_no_slash() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &token_id,
        &treasury,
        &1000,
        &dispute_contract,
        &0u64,
    );
    client.stake(&attestor, &1000);

    // Slash all stake
    env.as_contract(&dispute_contract, || {
        client.slash(&attestor, &1000, &1);
    });

    // Capture treasury balance before the NoSlash call (Req 2.5, Req 11.2)
    let treasury_balance_before = token_client.balance(&treasury);

    // Try to slash again with different dispute_id - should return NoSlash
    env.as_contract(&dispute_contract, || {
        let outcome = client.slash(&attestor, &500, &2);
        // Req 2.4: NoSlash returned when stake.amount == 0
        assert_eq!(outcome, SlashOutcome::NoSlash);
    });

    // Req 2.5 / Req 11.2: treasury balance must be unchanged on NoSlash
    assert_eq!(
        token_client.balance(&treasury),
        treasury_balance_before,
        "treasury balance must not change when NoSlash is returned"
    );
}

/// Test scenario: Dispute resolved as Upheld -> Slashing triggered
#[test]
fn test_dispute_resolution_triggers_slashing() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &token_id,
        &treasury,
        &1000,
        &dispute_contract,
        &0u64,
    );
    client.stake(&attestor, &5000);

    let initial_treasury = token_client.balance(&treasury);

    // Simulate dispute resolution: dispute_id=42, slash 30% of stake
    let slash_amount = 1500;
    env.as_contract(&dispute_contract, || {
        let outcome = client.slash(&attestor, &slash_amount, &42);
        assert_eq!(outcome, SlashOutcome::Slashed);
    });
    assert_eq!(client.get_stake(&attestor).unwrap().amount, 3500);
    assert_eq!(
        token_client.balance(&treasury),
        initial_treasury + slash_amount
    );
}

/// Test scenario: Slash with amount = 0 panics with correct message (Req 7.1)
#[test]
#[should_panic(expected = "slash amount must be positive")]
fn test_slash_amount_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let (attestor, _treasury, dispute_contract, _token_id, client) = setup(&env, 5000);

    env.as_contract(&dispute_contract, || {
        client.slash(&attestor, &0, &1);
    });
}

/// Test scenario: Zero-amount slash does not consume the dispute ID (Req 7.2, 7.3)
///
/// A `slash` call with `amount = 0` must panic before recording the dispute ID.
/// After the failed call, `stake.amount` must be unchanged and the same `dispute_id`
/// must still be usable for a subsequent valid slash.
#[test]
fn test_slash_zero_amount_does_not_consume_dispute_id() {
    let env = Env::default();
    env.mock_all_auths();

    let stake_amount = 5000_i128;
    let (attestor, _treasury, dispute_contract, _token_id, client) = setup(&env, stake_amount);

    let dispute_id: u64 = 42;

    // Attempt slash with amount = 0 — must panic (Req 7.1).
    // Use try_slash so the test can continue after the expected failure.
    let result = env.as_contract(&dispute_contract, || {
        client.try_slash(&attestor, &0, &dispute_id)
    });
    assert!(result.is_err(), "slash with amount=0 should panic");

    // Req 7.3: stake.amount must be unchanged after the failed call.
    let stake_after_failed = client.get_stake(&attestor).unwrap();
    assert_eq!(
        stake_after_failed.amount, stake_amount,
        "stake.amount must be unchanged after zero-amount slash"
    );

    // Req 7.2: the dispute_id was NOT consumed, so a valid slash with the same
    // dispute_id must succeed.
    let outcome = env.as_contract(&dispute_contract, || {
        client.slash(&attestor, &1000, &dispute_id)
    });
    assert_eq!(
        outcome,
        SlashOutcome::Slashed,
        "valid slash with previously-failed dispute_id should succeed"
    );

    // Confirm the stake was actually reduced by the valid slash.
    let stake_after_valid = client.get_stake(&attestor).unwrap();
    assert_eq!(
        stake_after_valid.amount,
        stake_amount - 1000,
        "stake.amount should reflect the valid slash"
    );
}

/// Test scenario: Slash reduces pending unstake to zero but preserves the record (Req 4.5)
///
/// After staking and requesting unstake for the full amount, slashing the full amount
/// must reduce `pending.amount` to 0 while keeping the `PendingUnstake` record present
/// so that `withdraw_unstaked` can still be called to clean up state.
#[test]
fn test_slash_pending_reduced_to_zero_record_preserved() {
    let env = Env::default();
    env.mock_all_auths();

    let stake_amount = 5000_i128;
    let (attestor, _treasury, dispute_contract, _token_id, client) = setup(&env, stake_amount);

    // Request unstake for the full staked amount
    client.request_unstake(&attestor, &stake_amount);

    // Confirm pending unstake exists with the full amount
    let pending_before = client.get_pending_unstake(&attestor).unwrap();
    assert_eq!(pending_before.amount, stake_amount);

    // Slash the full amount — this should reduce pending.amount to 0
    env.as_contract(&dispute_contract, || {
        let outcome = client.slash(&attestor, &stake_amount, &1);
        assert_eq!(outcome, SlashOutcome::Slashed);
    });

    // Req 4.5: PendingUnstake record must still exist (Some), with amount == 0
    let pending_after = client.get_pending_unstake(&attestor);
    assert!(
        pending_after.is_some(),
        "PendingUnstake record must be preserved after slash reduces it to zero"
    );
    assert_eq!(
        pending_after.unwrap().amount,
        0,
        "pending.amount must be 0 after full slash"
    );

    // Confirm stake.amount is also 0
    let stake_after = client.get_stake(&attestor).unwrap();
    assert_eq!(stake_after.amount, 0);
    assert_eq!(stake_after.locked, 0);
}

/// Test scenario: Withdraw after pending unstake is slashed to zero transfers 0 tokens (Req 9.3)
///
/// After staking, requesting unstake for the full amount, and slashing the full amount,
/// calling `withdraw_unstaked` after the unbonding period must:
/// - Transfer exactly 0 tokens to the attestor
/// - Clean up the PendingUnstake record
#[test]
fn test_slash_pending_zero_then_withdraw() {
    let env = Env::default();
    env.mock_all_auths();

    let stake_amount = 5000_i128;
    let (attestor, _treasury, dispute_contract, token_id, client) = setup(&env, stake_amount);

    let token_client = token::Client::new(&env, &token_id);

    // Request unstake for the full staked amount
    client.request_unstake(&attestor, &stake_amount);

    // Slash the full amount — reduces pending.amount to 0
    env.as_contract(&dispute_contract, || {
        let outcome = client.slash(&attestor, &stake_amount, &1);
        assert_eq!(outcome, SlashOutcome::Slashed);
    });

    // Confirm pending record exists with amount == 0 (Req 4.5)
    let pending = client.get_pending_unstake(&attestor).unwrap();
    assert_eq!(pending.amount, 0);

    // Advance ledger past the unbonding period unlock timestamp
    let unlock_ts = pending.unlock_timestamp;
    env.ledger().set_timestamp(unlock_ts + 1);

    // Record attestor balance before withdrawal
    let attestor_balance_before = token_client.balance(&attestor);

    // Call withdraw_unstaked — should transfer 0 tokens and clean up the record
    client.withdraw_unstaked(&attestor);

    // Req 9.3: 0 tokens transferred to attestor
    let attestor_balance_after = token_client.balance(&attestor);
    assert_eq!(
        attestor_balance_after,
        attestor_balance_before,
        "attestor should receive 0 tokens when pending.amount is 0"
    );

    // Req 9.3: pending unstake record cleaned up
    assert!(
        client.get_pending_unstake(&attestor).is_none(),
        "PendingUnstake record must be removed after withdraw_unstaked"
    );
}

/// Test scenario: Slash after withdraw_unstaked only affects remaining stake (Req 9.4)
///
/// After staking, requesting unstake for a partial amount, advancing the ledger,
/// and withdrawing the unstaked tokens, slashing the remaining stake must:
/// - Only reduce the remaining `stake.amount` (not the already-withdrawn amount)
/// - Leave the attestor's withdrawn balance unaffected
#[test]
fn test_slash_after_withdraw_unstaked() {
    let env = Env::default();
    env.mock_all_auths();

    let stake_amount = 5000_i128;
    let unstake_amount = 2000_i128;
    let remaining_stake = stake_amount - unstake_amount; // 3000
    let slash_amount = 1000_i128;

    let (attestor, _treasury, dispute_contract, token_id, client) = setup(&env, stake_amount);
    let token_client = token::Client::new(&env, &token_id);

    // Step 1: Request unstake for a partial amount
    client.request_unstake(&attestor, &unstake_amount);

    let pending = client.get_pending_unstake(&attestor).unwrap();
    assert_eq!(pending.amount, unstake_amount);

    // Step 2: Advance ledger past the unbonding period (unbonding_period = 0, so already unlocked)
    env.ledger().set_timestamp(pending.unlock_timestamp + 1);

    // Step 3: Withdraw the unstaked tokens
    let attestor_balance_before_withdraw = token_client.balance(&attestor);
    client.withdraw_unstaked(&attestor);
    let attestor_balance_after_withdraw = token_client.balance(&attestor);

    // Confirm withdrawal transferred the correct amount
    assert_eq!(
        attestor_balance_after_withdraw,
        attestor_balance_before_withdraw + unstake_amount,
        "withdraw_unstaked should transfer exactly unstake_amount to attestor"
    );

    // Confirm pending record is cleaned up
    assert!(
        client.get_pending_unstake(&attestor).is_none(),
        "PendingUnstake record must be removed after withdraw_unstaked"
    );

    // Confirm remaining stake
    let stake_after_withdraw = client.get_stake(&attestor).unwrap();
    assert_eq!(
        stake_after_withdraw.amount, remaining_stake,
        "stake.amount should equal stake_amount - unstake_amount after withdrawal"
    );
    assert_eq!(
        stake_after_withdraw.locked, 0,
        "stake.locked should be 0 after withdrawal"
    );

    // Step 4: Slash the remaining stake
    env.as_contract(&dispute_contract, || {
        let outcome = client.slash(&attestor, &slash_amount, &1);
        assert_eq!(outcome, SlashOutcome::Slashed);
    });

    // Req 9.4: slash only affects the remaining stake.amount
    let stake_after_slash = client.get_stake(&attestor).unwrap();
    assert_eq!(
        stake_after_slash.amount,
        remaining_stake - slash_amount,
        "slash must only reduce the remaining stake, not the already-withdrawn amount"
    );

    // Req 9.4: the already-withdrawn amount is unaffected — attestor's token balance unchanged
    assert_eq!(
        token_client.balance(&attestor),
        attestor_balance_after_withdraw,
        "attestor's withdrawn balance must not be affected by the subsequent slash"
    );
}

/// Test scenario: Frivolous slashing attempt (unauthorized caller)
#[test]
#[should_panic]
fn test_frivolous_slashing_blocked() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());
    let malicious_caller = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, _token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &token_id,
        &treasury,
        &1000,
        &dispute_contract,
        &0u64,
    );
    client.stake(&attestor, &5000);

    // Malicious caller tries to slash by impersonating a non-dispute contract.
    env.as_contract(&malicious_caller, || {
        client.slash(&attestor, &2000, &99);
    });
}

#[test]
fn test_slash_with_pending_unstake_adjusts_locked_and_pending() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, _token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(&admin, &token_id, &treasury, &1000, &dispute_contract, &0u64);
    client.stake(&attestor, &10000);
    client.request_unstake(&attestor, &6000);

    // Slash amount that forces stake.amount < previously locked value (6000 -> 4000 remains)
    env.as_contract(&dispute_contract, || {
        let outcome = client.slash(&attestor, &8000, &123);
        assert_eq!(outcome, SlashOutcome::Slashed);
    });

    let stake = client.get_stake(&attestor).unwrap();
    assert_eq!(stake.amount, 2000);
    assert_eq!(stake.locked, 2000);

    let pending = client.get_pending_unstake(&attestor).unwrap();
    assert_eq!(pending.amount, 2000);

    assert!(client.is_dispute_processed(&123));
}

#[test]
fn test_set_dispute_contract_updates_authorized_slasher() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let original_dispute_contract = env.register(DummyDisputeContract, ());
    let new_dispute_contract = env.register(DummyDisputeContract, ());

    let (token_id, token_admin, _token_client) = create_token_contract(&env, &admin);
    token_admin.mint(&attestor, &10000);

    let contract_id = env.register(AttestorStakingContract, ());
    let client = AttestorStakingContractClient::new(&env, &contract_id);

    client.initialize(&admin, &token_id, &treasury, &1000, &original_dispute_contract, &0u64);
    client.stake(&attestor, &5000);

    // old dispute contract cannot slash now after reconfiguration
    client.set_dispute_contract(&admin, &new_dispute_contract);

    env.as_contract(&original_dispute_contract, || {
        let result = client.try_slash(&attestor, &1000, &200);
        assert!(result.is_err());
    });

    // new dispute contract should be effective
    env.as_contract(&new_dispute_contract, || {
        let outcome = client.slash(&attestor, &1000, &200);
        assert_eq!(outcome, SlashOutcome::Slashed);
    });
}
