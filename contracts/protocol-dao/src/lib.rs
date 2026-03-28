#![no_std]
//! # Protocol DAO Governance Contract
//!
//! This contract provides DAO-style governance for Veritasor protocol parameters,
//! with comprehensive quorum enforcement mechanisms.
//!
//! ## Quorum Requirements
//!
//! - **Absolute Quorum**: Minimum total votes (for + against) must be >= `min_votes`
//! - **Majority**: Votes for must be strictly greater than votes against
//! - **Duration**: Proposals expire after `proposal_duration` ledger sequences
//!
//! ## Quorum Edge Cases
//!
//! - When `min_votes = 0`: Any proposal with at least 1 "for" vote and 0 "against" votes passes
//! - When `min_votes = 1`: Single voter can approve (1 for, 0 against satisfies quorum)
//! - Expired proposals cannot be voted on or executed
//! - Double-voting is prevented (one vote per voter per proposal)

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env};

// ════════════════════════════════════════════════════════════════════
// Data Structures
// ════════════════════════════════════════════════════════════════════

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract DAO admin address
    Admin,
    /// Optional governance token for vote gating (must hold balance to vote/propose)
    GovernanceToken,
    /// Minimum total votes required for quorum (for + against)
    MinVotes,
    /// Proposal lifetime in ledger sequences
    ProposalDuration,
    /// Next proposal ID counter
    NextProposalId,
    /// Proposal storage by ID
    Proposal(u64),
    /// Votes in favor for a proposal ID
    VotesFor(u64),
    /// Votes against for a proposal ID
    VotesAgainst(u64),
    /// Track if an address has voted on a proposal (prevents double-voting)
    HasVoted(u64, Address),
    /// Stored attestation fee configuration from executed proposals
    AttestationFeeConfig,
}

/// Proposal status lifecycle
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalStatus {
    /// Proposal is active and accepting votes
    Pending,
    /// Proposal was executed with quorum and majority
    Executed,
    /// Proposal was canceled or rejected
    Rejected,
    /// Proposal expired (no longer votable or executable)
    Expired,
}

/// Actions that can be performed via governance proposals
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalAction {
    /// Set attestation fee configuration (token, collector, base_fee, enabled)
    SetAttestationFeeConfig(Address, Address, i128, bool),
    /// Toggle attestation fee enabled flag
    SetAttestationFeeEnabled(bool),
    /// Update DAO governance parameters (min_votes, proposal_duration)
    UpdateGovernanceConfig(u32, u32),
}

/// Proposal structure containing governance action and voting metadata
#[contracttype]
#[derive(Clone, Debug)]
pub struct Proposal {
    /// Unique proposal identifier
    pub id: u64,
    /// Address that created this proposal (must authorize creation)
    pub creator: Address,
    /// Governance action to execute if proposal passes
    pub action: ProposalAction,
    /// Current proposal status
    pub status: ProposalStatus,
    /// Ledger sequence when proposal was created (used for expiry calculation)
    pub created_at: u32,
}

// ════════════════════════════════════════════════════════════════════
// Quorum & Validation Constants
// ════════════════════════════════════════════════════════════════════

/// Default minimum votes for quorum (can be overridden during initialization)
const DEFAULT_MIN_VOTES: u32 = 1;
/// Default proposal duration in ledger sequences (can be overridden during initialization)
const DEFAULT_PROPOSAL_DURATION: u32 = 120_960;
/// Maximum allowed min_votes to prevent unrealistic quorum requirements
const MAX_MIN_VOTES: u32 = 1_000_000;
/// Maximum allowed proposal duration to prevent indefinite voting periods
const MAX_PROPOSAL_DURATION: u32 = u32::MAX;

// ════════════════════════════════════════════════════════════════════
// Admin & Authorization
// ════════════════════════════════════════════════════════════════════

/// Retrieve the DAO admin address.
/// Panics if DAO is not initialized.
fn get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("dao not initialized")
}

/// Require caller is the DAO admin.
/// Panics if caller is not authorized or is not the admin.
fn require_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    let admin = get_admin(env);
    assert!(*caller == admin, "caller is not admin");
}

// ════════════════════════════════════════════════════════════════════
// Configuration Getters
// ════════════════════════════════════════════════════════════════════

/// Get the minimum votes required for quorum.
/// Returns DEFAULT_MIN_VOTES if not set.
fn get_min_votes(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MinVotes)
        .unwrap_or(DEFAULT_MIN_VOTES)
}

/// Get the proposal duration in ledger sequences.
/// Returns DEFAULT_PROPOSAL_DURATION if not set.
fn get_proposal_duration(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ProposalDuration)
        .unwrap_or(DEFAULT_PROPOSAL_DURATION)
}

/// Get the optional governance token (if vote gating is enabled).
fn get_governance_token(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::GovernanceToken)
}

// ════════════════════════════════════════════════════════════════════
// Token Gating
// ════════════════════════════════════════════════════════════════════

/// Check if caller holds governance token (if configured).
/// Panics if governance token is required but caller has no balance.
fn ensure_token_holder(env: &Env, who: &Address) {
    if let Some(token_addr) = get_governance_token(env) {
        let client = token::Client::new(env, &token_addr);
        let balance = client.balance(who);
        assert!(balance > 0, "insufficient governance token balance");
    }
}

// ════════════════════════════════════════════════════════════════════
// Proposal ID Management
// ════════════════════════════════════════════════════════════════════

/// Generate the next unique proposal ID.
/// Increments the counter and returns the new ID.
fn next_proposal_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::NextProposalId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::NextProposalId, &(id + 1));
    id
}

// ════════════════════════════════════════════════════════════════════
// Proposal Storage & Retrieval
// ════════════════════════════════════════════════════════════════════

/// Store a proposal in persistent storage.
fn store_proposal(env: &Env, proposal: &Proposal) {
    env.storage()
        .instance()
        .set(&DataKey::Proposal(proposal.id), proposal);
}

/// Retrieve a proposal by ID.
/// Panics if proposal does not exist.
fn get_proposal_internal(env: &Env, id: u64) -> Proposal {
    env.storage()
        .instance()
        .get(&DataKey::Proposal(id))
        .expect("proposal not found")
}

// ════════════════════════════════════════════════════════════════════
// Proposal Expiry
// ════════════════════════════════════════════════════════════════════

/// Check if a proposal has expired based on current ledger sequence.
/// A proposal expires when: current_sequence > created_at + duration
///
/// # Edge Cases
/// - If proposal_duration is 0, proposal expires immediately (next ledger)
/// - If proposal_duration is u32::MAX, proposal effectively never expires
fn is_expired(env: &Env, id: u64) -> bool {
    let proposal = get_proposal_internal(env, id);
    let duration = get_proposal_duration(env);
    // Saturating add prevents overflow; if it overflows, proposal never expires
    let expiry = proposal.created_at.saturating_add(duration);
    env.ledger().sequence() > expiry
}

// ════════════════════════════════════════════════════════════════════
// Vote Management
// ════════════════════════════════════════════════════════════════════

/// Get vote counts for a proposal (votes_for, votes_against).
/// Returns (0, 0) if no votes have been cast.
fn get_votes(env: &Env, id: u64) -> (u32, u32) {
    let for_votes: u32 = env
        .storage()
        .instance()
        .get(&DataKey::VotesFor(id))
        .unwrap_or(0);
    let against_votes: u32 = env
        .storage()
        .instance()
        .get(&DataKey::VotesAgainst(id))
        .unwrap_or(0);
    (for_votes, against_votes)
}

/// Check if a voter has already voted on a proposal.
fn has_voted(env: &Env, id: u64, voter: &Address) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::HasVoted(id, voter.clone()))
        .unwrap_or(false)
}

/// Record that a voter has voted on a proposal (prevents double-voting).
fn set_voted(env: &Env, id: u64, voter: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::HasVoted(id, voter.clone()), &true);
}

/// Increment votes in favor for a proposal.
fn increment_for(env: &Env, id: u64) {
    let (for_votes, _) = get_votes(env, id);
    env.storage()
        .instance()
        .set(&DataKey::VotesFor(id), &(for_votes.saturating_add(1)));
}

/// Increment votes against for a proposal.
fn increment_against(env: &Env, id: u64) {
    let (_, against_votes) = get_votes(env, id);
    env.storage()
        .instance()
        .set(&DataKey::VotesAgainst(id), &(against_votes.saturating_add(1)));
}

// ════════════════════════════════════════════════════════════════════
// Quorum Checking
// ════════════════════════════════════════════════════════════════════

/// Check if quorum has been met for a proposal.
///
/// Quorum is met when: votes_for + votes_against >= min_votes
///
/// # Edge Cases
/// - If min_votes = 0: quorum is always met (total can be 0)
/// - If no votes cast yet: needs at least min_votes votes to reach quorum
/// - Ties do NOT satisfy quorum requirement (votes are separate from majority)
fn quorum_met(env: &Env, id: u64) -> bool {
    let (for_votes, against_votes) = get_votes(env, id);
    let total = for_votes.saturating_add(against_votes);
    total >= get_min_votes(env)
}

/// Check if a proposal has majority approval (votes_for > votes_against).
fn has_majority(env: &Env, id: u64) -> bool {
    let (for_votes, against_votes) = get_votes(env, id);
    for_votes > against_votes
}

/// Get quorum status for a proposal: (votes_for, votes_against, min_required, quorum_met, majority).
/// Useful for off-chain queries to determine proposal state.
fn get_quorum_status(env: &Env, id: u64) -> (u32, u32, u32, bool, bool) {
    let (for_votes, against_votes) = get_votes(env, id);
    let min_required = get_min_votes(env);
    let quorum_ok = quorum_met(env, id);
    let majority_ok = has_majority(env, id);
    (for_votes, against_votes, min_required, quorum_ok, majority_ok)
}

// ════════════════════════════════════════════════════════════════════
// Proposal Action Execution
// ════════════════════════════════════════════════════════════════════

/// Apply a proposal action to contract state if execution succeeds.
fn apply_action(env: &Env, action: &ProposalAction) {
    match action {
        ProposalAction::SetAttestationFeeConfig(token, collector, base_fee, enabled) => {
            let cfg: (Address, Address, i128, bool) =
                (token.clone(), collector.clone(), *base_fee, *enabled);
            env.storage()
                .instance()
                .set(&DataKey::AttestationFeeConfig, &cfg);
        }
        ProposalAction::SetAttestationFeeEnabled(enabled) => {
            let mut cfg: (Address, Address, i128, bool) = env
                .storage()
                .instance()
                .get(&DataKey::AttestationFeeConfig)
                .expect("attestation fee config not set");
            cfg.3 = *enabled;
            env.storage()
                .instance()
                .set(&DataKey::AttestationFeeConfig, &cfg);
        }
        ProposalAction::UpdateGovernanceConfig(min_votes, duration) => {
            env.storage().instance().set(&DataKey::MinVotes, min_votes);
            env.storage()
                .instance()
                .set(&DataKey::ProposalDuration, duration);
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Validation Functions
// ════════════════════════════════════════════════════════════════════

/// Validate min_votes parameter is within acceptable range.
/// Panics if min_votes exceeds MAX_MIN_VOTES.
fn validate_min_votes(min_votes: u32) {
    assert!(
        min_votes <= MAX_MIN_VOTES,
        "min_votes exceeds maximum allowed value"
    );
}

/// Validate proposal_duration parameter is within acceptable range.
/// Panics if proposal_duration exceeds MAX_PROPOSAL_DURATION.
fn validate_proposal_duration(duration: u32) {
    assert!(
        duration <= MAX_PROPOSAL_DURATION,
        "proposal_duration exceeds maximum allowed value"
    );
}

#[contract]
pub struct ProtocolDao;

#[contractimpl]
impl ProtocolDao {
    /// Initialize the DAO with admin, governance token, and quorum parameters.
    ///
    /// # Arguments
    ///
    /// * `admin` - DAO administrator address (must authorize this call)
    /// * `governance_token` - Optional token for vote gating (if Some, callers must hold > 0 balance)
    /// * `min_votes` - Minimum total votes for quorum (0 defaults to DEFAULT_MIN_VOTES)
    /// * `proposal_duration` - Proposal lifetime in ledger sequences (0 defaults to DEFAULT_PROPOSAL_DURATION)
    ///
    /// # Panics
    ///
    /// * If DAO is already initialized
    /// * If `admin` does not authorize the call
    /// * If `min_votes > MAX_MIN_VOTES`
    /// * If `proposal_duration > MAX_PROPOSAL_DURATION`
    ///
    /// # Quorum Edge Cases
    ///
    /// - `min_votes = 0`: Quorum always met (any total >= 0 satisfies)
    /// - `min_votes = 1`: Single voter can approve with (1 for, 0 against)
    pub fn initialize(
        env: Env,
        admin: Address,
        governance_token: Option<Address>,
        min_votes: u32,
        proposal_duration: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();

        // Validate parameters
        validate_min_votes(min_votes);
        validate_proposal_duration(proposal_duration);

        env.storage().instance().set(&DataKey::Admin, &admin);

        if let Some(token_addr) = governance_token {
            env.storage()
                .instance()
                .set(&DataKey::GovernanceToken, &token_addr);
        }

        let mv = if min_votes == 0 {
            DEFAULT_MIN_VOTES
        } else {
            min_votes
        };
        let dur = if proposal_duration == 0 {
            DEFAULT_PROPOSAL_DURATION
        } else {
            proposal_duration
        };

        env.storage().instance().set(&DataKey::MinVotes, &mv);
        env.storage()
            .instance()
            .set(&DataKey::ProposalDuration, &dur);
    }

    /// Set or change the governance token for vote gating.
    ///
    /// # Authorization
    ///
    /// Only the DAO admin can call this function.
    ///
    /// # Effects
    ///
    /// - Updates the governance token address
    /// - If set to a new token, future votes require balance in that token
    /// - Existing votes are not affected
    pub fn set_governance_token(env: Env, caller: Address, token: Address) {
        require_admin(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::GovernanceToken, &token);
    }

    /// Update DAO voting configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `min_votes` - New minimum votes for quorum (0 defaults to DEFAULT_MIN_VOTES)
    /// * `proposal_duration` - New proposal lifetime in ledger sequences (0 defaults to DEFAULT_PROPOSAL_DURATION)
    ///
    /// # Authorization
    ///
    /// Only the DAO admin can call this function.
    ///
    /// # Panics
    ///
    /// * If `min_votes > MAX_MIN_VOTES`
    /// * If `proposal_duration > MAX_PROPOSAL_DURATION`
    ///
    /// # Effects
    ///
    /// - Updates voting configuration
    /// - Change applies only to new proposals
    /// - Existing pending proposals retain their original quorum values
    pub fn set_voting_config(env: Env, caller: Address, min_votes: u32, proposal_duration: u32) {
        require_admin(&env, &caller);

        // Validate parameters
        validate_min_votes(min_votes);
        validate_proposal_duration(proposal_duration);

        let mv = if min_votes == 0 {
            DEFAULT_MIN_VOTES
        } else {
            min_votes
        };
        let dur = if proposal_duration == 0 {
            DEFAULT_PROPOSAL_DURATION
        } else {
            proposal_duration
        };
        env.storage().instance().set(&DataKey::MinVotes, &mv);
        env.storage()
            .instance()
            .set(&DataKey::ProposalDuration, &dur);
    }

    /// Create a proposal to set attestation fee configuration.
    ///
    /// # Authorization
    ///
    /// * Requires `creator` to authorize the call
    /// * If governance token is configured, creator must hold a positive balance
    ///
    /// # Arguments
    ///
    /// * `token` - Fee token address
    /// * `collector` - Fee collector address
    /// * `base_fee` - Base fee amount (must be non-negative)
    /// * `enabled` - Whether fees are enabled
    ///
    /// # Panics
    ///
    /// * If `base_fee < 0`
    /// * If governance gating is enabled and creator has no token balance
    ///
    /// # Returns
    ///
    /// Unique proposal ID that can be used for voting and execution
    pub fn create_fee_config_proposal(
        env: Env,
        creator: Address,
        token: Address,
        collector: Address,
        base_fee: i128,
        enabled: bool,
    ) -> u64 {
        creator.require_auth();
        ensure_token_holder(&env, &creator);

        assert!(base_fee >= 0, "base_fee must be non-negative");

        let id = next_proposal_id(&env);
        let proposal = Proposal {
            id,
            creator: creator.clone(),
            action: ProposalAction::SetAttestationFeeConfig(token, collector, base_fee, enabled),
            status: ProposalStatus::Pending,
            created_at: env.ledger().sequence(),
        };
        store_proposal(&env, &proposal);
        id
    }

    /// Create a proposal to toggle the attestation fee enabled flag.
    ///
    /// # Authorization
    ///
    /// * Requires `creator` to authorize the call
    /// * If governance token is configured, creator must hold a positive balance
    ///
    /// # Arguments
    ///
    /// * `enabled` - New enabled flag for attestation fees
    ///
    /// # Returns
    ///
    /// Unique proposal ID
    pub fn create_fee_toggle_proposal(env: Env, creator: Address, enabled: bool) -> u64 {
        creator.require_auth();
        ensure_token_holder(&env, &creator);

        let id = next_proposal_id(&env);
        let proposal = Proposal {
            id,
            creator: creator.clone(),
            action: ProposalAction::SetAttestationFeeEnabled(enabled),
            status: ProposalStatus::Pending,
            created_at: env.ledger().sequence(),
        };
        store_proposal(&env, &proposal);
        id
    }

    /// Create a proposal to update DAO governance configuration.
    ///
    /// # Authorization
    ///
    /// * Requires `creator` to authorize the call
    /// * If governance token is configured, creator must hold a positive balance
    ///
    /// # Arguments
    ///
    /// * `min_votes` - Proposed minimum votes for quorum
    /// * `proposal_duration` - Proposed proposal duration in ledger sequences
    ///
    /// # Returns
    ///
    /// Unique proposal ID
    pub fn create_gov_config_proposal(
        env: Env,
        creator: Address,
        min_votes: u32,
        proposal_duration: u32,
    ) -> u64 {
        creator.require_auth();
        ensure_token_holder(&env, &creator);

        let id = next_proposal_id(&env);
        let proposal = Proposal {
            id,
            creator: creator.clone(),
            action: ProposalAction::UpdateGovernanceConfig(min_votes, proposal_duration),
            status: ProposalStatus::Pending,
            created_at: env.ledger().sequence(),
        };
        store_proposal(&env, &proposal);
        id
    }

    /// Vote in favor of a proposal.
    ///
    /// # Authorization
    ///
    /// * Requires `voter` to authorize the call
    /// * If governance token is configured, voter must hold a positive balance
    /// * Caller cannot vote multiple times on the same proposal
    /// * Proposal must be pending and not expired
    ///
    /// # Panics
    ///
    /// * If proposal is not in Pending status
    /// * If proposal has expired
    /// * If voter has already voted on this proposal
    /// * If governance gating is enabled and voter has no token balance
    pub fn vote_for(env: Env, voter: Address, id: u64) {
        voter.require_auth();
        ensure_token_holder(&env, &voter);

        let proposal = get_proposal_internal(&env, id);
        assert!(
            proposal.status == ProposalStatus::Pending,
            "proposal is not pending"
        );
        assert!(!is_expired(&env, id), "proposal expired");
        assert!(!has_voted(&env, id, &voter), "already voted");

        increment_for(&env, id);
        set_voted(&env, id, &voter);
        store_proposal(&env, &proposal);
    }

    /// Vote against a proposal.
    ///
    /// # Authorization
    ///
    /// * Requires `voter` to authorize the call
    /// * If governance token is configured, voter must hold a positive balance
    /// * Caller cannot vote multiple times on the same proposal
    /// * Proposal must be pending and not expired
    ///
    /// # Panics
    ///
    /// * If proposal is not in Pending status
    /// * If proposal has expired
    /// * If voter has already voted on this proposal
    /// * If governance gating is enabled and voter has no token balance
    pub fn vote_against(env: Env, voter: Address, id: u64) {
        voter.require_auth();
        ensure_token_holder(&env, &voter);

        let proposal = get_proposal_internal(&env, id);
        assert!(
            proposal.status == ProposalStatus::Pending,
            "proposal is not pending"
        );
        assert!(!is_expired(&env, id), "proposal expired");
        assert!(!has_voted(&env, id, &voter), "already voted");

        increment_against(&env, id);
        set_voted(&env, id, &voter);
        store_proposal(&env, &proposal);
    }

    /// Execute a proposal if it meets quorum and majority requirements.
    ///
    /// # Execution Requirements
    ///
    /// 1. Proposal must be in Pending status
    /// 2. Proposal must not be expired
    /// 3. Quorum must be met: `votes_for + votes_against >= min_votes`
    /// 4. Majority must exist: `votes_for > votes_against`
    ///
    /// # Quorum Edge Cases
    ///
    /// - If `min_votes = 0` and `votes_for = 1, votes_against = 0`: quorum met and majority satisfied
    /// - If `votes_for = votes_against`: majority check fails (tie vote rejected)
    /// - Proposal must have at least 1 vote to have quorum > 0
    ///
    /// # Panics
    ///
    /// * If proposal not in Pending status
    /// * If proposal is expired
    /// * If quorum not met
    /// * If strict majority not achieved
    ///
    /// # Effects
    ///
    /// * Applies the proposal action to contract state
    /// * Sets proposal status to Executed
    pub fn execute_proposal(env: Env, executor: Address, id: u64) {
        executor.require_auth();

        let mut proposal = get_proposal_internal(&env, id);
        assert!(
            proposal.status == ProposalStatus::Pending,
            "proposal is not pending"
        );
        assert!(!is_expired(&env, id), "proposal expired");
        assert!(quorum_met(&env, id), "quorum not met");
        assert!(has_majority(&env, id), "proposal not approved");

        apply_action(&env, &proposal.action);

        proposal.status = ProposalStatus::Executed;
        store_proposal(&env, &proposal);
    }

    /// Cancel a pending proposal.
    ///
    /// # Authorization
    ///
    /// Only the proposal creator or DAO admin can cancel a proposal.
    ///
    /// # Panics
    ///
    /// * If proposal is not in Pending status
    /// * If caller is neither creator nor admin
    ///
    /// # Effects
    ///
    /// * Sets proposal status to Rejected
    /// * Cancels all voting on the proposal
    pub fn cancel_proposal(env: Env, caller: Address, id: u64) {
        caller.require_auth();

        let mut proposal = get_proposal_internal(&env, id);
        assert!(
            proposal.status == ProposalStatus::Pending,
            "proposal is not pending"
        );
        assert!(
            proposal.creator == caller || get_admin(&env) == caller,
            "only creator or admin can cancel"
        );

        proposal.status = ProposalStatus::Rejected;
        store_proposal(&env, &proposal);
    }

    /// Get proposal details by ID.
    ///
    /// # Returns
    ///
    /// `Option<Proposal>` containing the proposal if it exists, None otherwise.
    pub fn get_proposal(env: Env, id: u64) -> Option<Proposal> {
        env.storage().instance().get(&DataKey::Proposal(id))
    }

    /// Get the number of votes in favor for a proposal.
    pub fn get_votes_for(env: Env, id: u64) -> u32 {
        let (for_votes, _) = get_votes(&env, id);
        for_votes
    }

    /// Get the number of votes against for a proposal.
    pub fn get_votes_against(env: Env, id: u64) -> u32 {
        let (_, against_votes) = get_votes(&env, id);
        against_votes
    }

    /// Get current DAO configuration.
    ///
    /// # Returns
    ///
    /// Tuple of `(admin, governance_token, min_votes, proposal_duration)`
    pub fn get_config(env: Env) -> (Address, Option<Address>, u32, u32) {
        let admin = get_admin(&env);
        let token = get_governance_token(&env);
        let min_votes = get_min_votes(&env);
        let duration = get_proposal_duration(&env);
        (admin, token, min_votes, duration)
    }

    /// Get quorum and voting status for a proposal.
    ///
    /// # Returns
    ///
    /// Tuple of `(votes_for, votes_against, min_required, quorum_met, majority_achieved)`
    /// Useful for off-chain UIs to display proposal status.
    pub fn get_quorum_info(env: Env, id: u64) -> (u32, u32, u32, bool, bool) {
        get_quorum_status(&env, id)
    }

    /// Get attestation fee configuration set by executed proposals.
    ///
    /// # Returns
    ///
    /// `Option<(token, collector, base_fee, enabled)>` if configuration has been set.
    pub fn get_attestation_fee_config(env: Env) -> Option<(Address, Address, i128, bool)> {
        env.storage().instance().get(&DataKey::AttestationFeeConfig)
    }
}

#[cfg(test)]
mod test;
