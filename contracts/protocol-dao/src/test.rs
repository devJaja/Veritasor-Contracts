extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env};

fn setup_with_token(
    min_votes: u32,
    proposal_duration: u32,
) -> (Env, ProtocolDaoClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = token_contract.address().clone();

    let admin = Address::generate(&env);
    let contract_id = env.register(ProtocolDao, ());
    let client = ProtocolDaoClient::new(&env, &contract_id);
    client.initialize(
        &admin,
        &Some(token_addr.clone()),
        &min_votes,
        &proposal_duration,
    );

    (env, client, admin, token_addr)
}

fn setup_without_token(
    min_votes: u32,
    proposal_duration: u32,
) -> (Env, ProtocolDaoClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(ProtocolDao, ());
    let client = ProtocolDaoClient::new(&env, &contract_id);
    client.initialize(&admin, &None, &min_votes, &proposal_duration);

    (env, client, admin)
}

fn mint(env: &Env, token_addr: &Address, to: &Address, amount: i128) {
    let stellar = StellarAssetClient::new(env, token_addr);
    stellar.mint(to, &amount);
}

#[test]
fn initialize_sets_defaults() {
    let (_env, client, admin, token_addr) = setup_with_token(0, 0);
    let (stored_admin, stored_token, min_votes, duration) = client.get_config();
    assert_eq!(stored_admin, admin);
    assert_eq!(stored_token, Some(token_addr));
    assert_eq!(min_votes, DEFAULT_MIN_VOTES);
    assert_eq!(duration, DEFAULT_PROPOSAL_DURATION);
}

#[test]
#[should_panic(expected = "already initialized")]
fn initialize_twice_panics() {
    let (_env, client, admin, token_addr) = setup_with_token(1, 10);
    client.initialize(&admin, &Some(token_addr), &1, &10);
}

#[test]
fn set_governance_token_by_admin() {
    let (env, client, admin, _token_addr) = setup_with_token(1, 10);
    let new_token = Address::generate(&env);
    client.set_governance_token(&admin, &new_token);
    let (_, stored_token, _, _) = client.get_config();
    assert_eq!(stored_token, Some(new_token));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn set_governance_token_by_non_admin_panics() {
    let (env, client, _admin, _token_addr) = setup_with_token(1, 10);
    let caller = Address::generate(&env);
    let new_token = Address::generate(&env);
    client.set_governance_token(&caller, &new_token);
}

#[test]
fn set_voting_config_by_admin() {
    let (_env, client, admin, _token_addr) = setup_with_token(1, 10);
    client.set_voting_config(&admin, &3, &20);
    let (_, _, min_votes, duration) = client.get_config();
    assert_eq!(min_votes, 3);
    assert_eq!(duration, 20);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn set_voting_config_by_non_admin_panics() {
    let (env, client, _admin, _token_addr) = setup_with_token(1, 10);
    let caller = Address::generate(&env);
    client.set_voting_config(&caller, &3, &20);
}

#[test]
fn create_and_execute_fee_config_proposal() {
    let (env, client, admin, gov_token) = setup_with_token(1, 100);

    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&voter, &fee_token, &collector, &1_000, &true);

    client.vote_for(&voter, &proposal_id);

    client.execute_proposal(&admin, &proposal_id);

    let proposal = client.get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Executed);

    let cfg = client.get_attestation_fee_config().unwrap();
    assert_eq!(cfg.0, fee_token);
    assert_eq!(cfg.1, collector);
    assert_eq!(cfg.2, 1_000);
    assert!(cfg.3);
}

#[test]
#[should_panic(expected = "insufficient governance token balance")]
fn create_proposal_without_token_panics() {
    let (env, client, _admin, _gov_token) = setup_with_token(1, 100);
    let voter = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    client.create_fee_config_proposal(&voter, &fee_token, &collector, &1_000, &true);
}

#[test]
fn create_proposal_without_governance_token_configured_allows_anyone() {
    let (env, client, _admin) = setup_without_token(1, 100);
    let voter = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&voter, &fee_token, &collector, &1_000, &true);
    client.vote_for(&voter, &proposal_id);
}

#[test]
fn quorum_and_majority_required() {
    let (env, client, admin, gov_token) = setup_with_token(2, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&voter1, &fee_token, &collector, &1_000, &true);

    client.vote_for(&voter1, &proposal_id);
    client.vote_for(&voter2, &proposal_id);

    let for_votes = client.get_votes_for(&proposal_id);
    let against_votes = client.get_votes_against(&proposal_id);
    assert_eq!(for_votes, 2);
    assert_eq!(against_votes, 0);

    client.execute_proposal(&admin, &proposal_id);
}

#[test]
#[should_panic(expected = "quorum not met")]
fn execute_without_quorum_panics() {
    let (env, client, admin, gov_token) = setup_with_token(2, 100);

    let voter1 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&voter1, &fee_token, &collector, &1_000, &true);

    client.vote_for(&voter1, &proposal_id);

    client.execute_proposal(&admin, &proposal_id);
}

#[test]
#[should_panic(expected = "proposal not approved")]
fn execute_with_tied_votes_panics() {
    let (env, client, admin, gov_token) = setup_with_token(2, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&voter1, &fee_token, &collector, &1_000, &true);

    client.vote_for(&voter1, &proposal_id);
    client.vote_against(&voter2, &proposal_id);

    client.execute_proposal(&admin, &proposal_id);
}

#[test]
fn cancel_proposal_by_creator() {
    let (env, client, _admin, gov_token) = setup_with_token(1, 100);

    let creator = Address::generate(&env);
    mint(&env, &gov_token, &creator, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&creator, &fee_token, &collector, &1_000, &true);

    client.cancel_proposal(&creator, &proposal_id);

    let proposal = client.get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Rejected);
}

#[test]
fn cancel_proposal_by_admin() {
    let (env, client, admin, gov_token) = setup_with_token(1, 100);

    let creator = Address::generate(&env);
    mint(&env, &gov_token, &creator, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&creator, &fee_token, &collector, &1_000, &true);

    client.cancel_proposal(&admin, &proposal_id);

    let proposal = client.get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Rejected);
}

#[test]
#[should_panic(expected = "only creator or admin can cancel")]
fn cancel_proposal_by_other_panics() {
    let (env, client, _admin, gov_token) = setup_with_token(1, 100);

    let creator = Address::generate(&env);
    mint(&env, &gov_token, &creator, 100);

    let other = Address::generate(&env);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&creator, &fee_token, &collector, &1_000, &true);

    client.cancel_proposal(&other, &proposal_id);
}

#[test]
#[should_panic(expected = "proposal expired")]
fn vote_after_expiry_panics() {
    let (env, client, _admin, gov_token) = setup_with_token(1, 5);

    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&voter, &fee_token, &collector, &1_000, &true);

    env.ledger().with_mut(|li| {
        li.sequence_number += 10;
    });

    client.vote_for(&voter, &proposal_id);
}

#[test]
#[should_panic(expected = "proposal expired")]
fn execute_after_expiry_panics() {
    let (env, client, admin, gov_token) = setup_with_token(1, 5);

    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);

    let proposal_id =
        client.create_fee_config_proposal(&voter, &fee_token, &collector, &1_000, &true);

    client.vote_for(&voter, &proposal_id);

    env.ledger().with_mut(|li| {
        li.sequence_number += 10;
    });

    client.execute_proposal(&admin, &proposal_id);
}

// ── Quorum Manipulation Tests ────────────────────────────────────────────────
//
// These tests verify that quorum cannot be gamed through:
//   - duplicate votes
//   - voting on non-pending proposals
//   - lowering quorum mid-flight via governance proposals
//   - raising quorum to block execution
//   - against-only votes satisfying quorum but failing majority
//   - quorum exactly at boundary (min_votes == total votes)
//   - gov-config proposals that self-referentially lower quorum

// Adversarial: duplicate vote must not count twice
#[test]
#[should_panic(expected = "already voted")]
fn duplicate_vote_for_panics() {
    let (env, client, _admin, gov_token) = setup_with_token(1, 100);
    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_for(&voter, &id);
    client.vote_for(&voter, &id); // must panic
}

// Adversarial: duplicate vote_against must not count twice
#[test]
#[should_panic(expected = "already voted")]
fn duplicate_vote_against_panics() {
    let (env, client, _admin, gov_token) = setup_with_token(1, 100);
    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_against(&voter, &id);
    client.vote_against(&voter, &id); // must panic
}

// Adversarial: voter cannot switch from for to against
#[test]
#[should_panic(expected = "already voted")]
fn switch_vote_from_for_to_against_panics() {
    let (env, client, _admin, gov_token) = setup_with_token(1, 100);
    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_for(&voter, &id);
    client.vote_against(&voter, &id); // must panic
}

// Adversarial: voter cannot switch from against to for
#[test]
#[should_panic(expected = "already voted")]
fn switch_vote_from_against_to_for_panics() {
    let (env, client, _admin, gov_token) = setup_with_token(1, 100);
    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_against(&voter, &id);
    client.vote_for(&voter, &id); // must panic
}

// Correctness: against-only votes satisfy quorum but must not pass majority
#[test]
#[should_panic(expected = "proposal not approved")]
fn quorum_met_by_against_votes_only_does_not_execute() {
    let (env, client, admin, gov_token) = setup_with_token(2, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter1, &fee_token, &collector, &500, &true);

    client.vote_against(&voter1, &id);
    client.vote_against(&voter2, &id);

    // quorum is met (2 >= 2) but for_votes(0) > against_votes(2) is false
    client.execute_proposal(&admin, &id);
}

// Correctness: quorum exactly at boundary executes successfully
#[test]
fn quorum_exactly_at_boundary_executes() {
    let (env, client, admin, gov_token) = setup_with_token(3, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);
    mint(&env, &gov_token, &voter3, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter1, &fee_token, &collector, &500, &true);

    client.vote_for(&voter1, &id);
    client.vote_for(&voter2, &id);
    client.vote_for(&voter3, &id);

    assert_eq!(client.get_votes_for(&id), 3);
    assert_eq!(client.get_votes_against(&id), 0);

    client.execute_proposal(&admin, &id);
    let proposal = client.get_proposal(&id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Executed);
}

// Correctness: one below quorum boundary must not execute
#[test]
#[should_panic(expected = "quorum not met")]
fn one_below_quorum_boundary_panics() {
    let (env, client, admin, gov_token) = setup_with_token(3, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter1, &fee_token, &collector, &500, &true);

    client.vote_for(&voter1, &id);
    client.vote_for(&voter2, &id);

    client.execute_proposal(&admin, &id);
}

// Adversarial: cannot vote on an already-executed proposal
#[test]
#[should_panic(expected = "proposal is not pending")]
fn vote_on_executed_proposal_panics() {
    let (env, client, admin, gov_token) = setup_with_token(1, 100);

    let voter = Address::generate(&env);
    let voter2 = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);
    mint(&env, &gov_token, &voter2, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_for(&voter, &id);
    client.execute_proposal(&admin, &id);

    client.vote_for(&voter2, &id); // must panic
}

// Adversarial: cannot vote on a cancelled/rejected proposal
#[test]
#[should_panic(expected = "proposal is not pending")]
fn vote_on_cancelled_proposal_panics() {
    let (env, client, admin, gov_token) = setup_with_token(1, 100);

    let creator = Address::generate(&env);
    mint(&env, &gov_token, &creator, 100);

    let voter2 = Address::generate(&env);
    mint(&env, &gov_token, &voter2, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&creator, &fee_token, &collector, &500, &true);

    client.cancel_proposal(&admin, &id);
    client.vote_for(&voter2, &id); // must panic
}

// Adversarial: cannot execute an already-executed proposal
#[test]
#[should_panic(expected = "proposal is not pending")]
fn execute_already_executed_proposal_panics() {
    let (env, client, admin, gov_token) = setup_with_token(1, 100);

    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_for(&voter, &id);
    client.execute_proposal(&admin, &id);
    client.execute_proposal(&admin, &id); // must panic
}

// Adversarial: cannot cancel an already-executed proposal
#[test]
#[should_panic(expected = "proposal is not pending")]
fn cancel_executed_proposal_panics() {
    let (env, client, admin, gov_token) = setup_with_token(1, 100);

    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_for(&voter, &id);
    client.execute_proposal(&admin, &id);
    client.cancel_proposal(&admin, &id); // must panic
}

// Regression: raising quorum via set_voting_config blocks a proposal that
// previously had enough votes
#[test]
#[should_panic(expected = "quorum not met")]
fn raising_quorum_after_votes_blocks_execution() {
    let (env, client, admin, gov_token) = setup_with_token(1, 100);

    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_for(&voter, &id);
    // admin raises quorum to 5 after the vote — execution must now fail
    client.set_voting_config(&admin, &5, &100);

    client.execute_proposal(&admin, &id);
}

// Regression: lowering quorum via set_voting_config allows a previously
// blocked proposal to execute
#[test]
fn lowering_quorum_unblocks_execution() {
    let (env, client, admin, gov_token) = setup_with_token(5, 100);

    let voter = Address::generate(&env);
    mint(&env, &gov_token, &voter, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter, &fee_token, &collector, &500, &true);

    client.vote_for(&voter, &id);
    // only 1 vote, quorum was 5 — lower it to 1
    client.set_voting_config(&admin, &1, &100);

    client.execute_proposal(&admin, &id);
    let proposal = client.get_proposal(&id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Executed);
}

// Correctness: gov-config proposal that lowers quorum takes effect and
// allows a subsequent proposal to execute with fewer votes
#[test]
fn gov_config_proposal_lowers_quorum_for_future_proposals() {
    let (env, client, admin, gov_token) = setup_with_token(3, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);
    mint(&env, &gov_token, &voter3, 100);

    // Proposal A: lower quorum from 3 → 1
    let gov_id = client.create_gov_config_proposal(&voter1, &1, &100);
    client.vote_for(&voter1, &gov_id);
    client.vote_for(&voter2, &gov_id);
    client.vote_for(&voter3, &gov_id);
    client.execute_proposal(&admin, &gov_id);

    let (_, _, min_votes, _) = client.get_config();
    assert_eq!(min_votes, 1);

    // Proposal B: now only 1 vote needed
    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let fee_id = client.create_fee_config_proposal(&voter1, &fee_token, &collector, &999, &true);
    client.vote_for(&voter1, &fee_id);
    client.execute_proposal(&admin, &fee_id);

    let proposal = client.get_proposal(&fee_id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Executed);
}

// Correctness: gov-config proposal that raises quorum blocks proposals
// that would have passed under the old quorum
#[test]
#[should_panic(expected = "quorum not met")]
fn gov_config_proposal_raises_quorum_blocks_execution() {
    let (env, client, admin, gov_token) = setup_with_token(1, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);
    mint(&env, &gov_token, &voter3, 100);

    // Proposal A: raise quorum from 1 → 5
    let gov_id = client.create_gov_config_proposal(&voter1, &5, &100);
    client.vote_for(&voter1, &gov_id);
    client.execute_proposal(&admin, &gov_id);

    let (_, _, min_votes, _) = client.get_config();
    assert_eq!(min_votes, 5);

    // Proposal B: only 1 vote — must fail quorum
    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let fee_id = client.create_fee_config_proposal(&voter2, &fee_token, &collector, &999, &true);
    client.vote_for(&voter2, &fee_id);
    client.execute_proposal(&admin, &fee_id);
}

// Adversarial: token-less voter cannot vote even if quorum is 1
#[test]
#[should_panic(expected = "insufficient governance token balance")]
fn voter_without_token_cannot_vote() {
    let (env, client, _admin, _gov_token) = setup_with_token(1, 100);

    // creator has token, voter2 does not
    let creator = Address::generate(&env);
    let voter2 = Address::generate(&env);
    mint(&env, &_gov_token, &creator, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&creator, &fee_token, &collector, &500, &true);

    client.vote_for(&voter2, &id); // must panic
}

// Correctness: mixed votes — majority for with quorum met executes
#[test]
fn majority_for_with_mixed_votes_executes() {
    let (env, client, admin, gov_token) = setup_with_token(3, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);
    mint(&env, &gov_token, &voter3, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter1, &fee_token, &collector, &500, &true);

    client.vote_for(&voter1, &id);
    client.vote_for(&voter2, &id);
    client.vote_against(&voter3, &id);

    assert_eq!(client.get_votes_for(&id), 2);
    assert_eq!(client.get_votes_against(&id), 1);

    client.execute_proposal(&admin, &id);
    let proposal = client.get_proposal(&id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Executed);
}

// Correctness: majority against with quorum met must not execute
#[test]
#[should_panic(expected = "proposal not approved")]
fn majority_against_with_quorum_met_does_not_execute() {
    let (env, client, admin, gov_token) = setup_with_token(3, 100);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);
    mint(&env, &gov_token, &voter1, 100);
    mint(&env, &gov_token, &voter2, 100);
    mint(&env, &gov_token, &voter3, 100);

    let fee_token = Address::generate(&env);
    let collector = Address::generate(&env);
    let id = client.create_fee_config_proposal(&voter1, &fee_token, &collector, &500, &true);

    client.vote_for(&voter1, &id);
    client.vote_against(&voter2, &id);
    client.vote_against(&voter3, &id);

    client.execute_proposal(&admin, &id);
}
