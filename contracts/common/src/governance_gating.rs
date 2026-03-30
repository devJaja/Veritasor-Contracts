//! Shared governance gating helpers for contracts that need token-threshold
//! authorization, delegation, and stricter controls for role-escalation flows.
//!
//! ## Design goals
//!
//! - Keep ordinary governance actions token-threshold gated.
//! - Allow delegated voting power for normal governance by default.
//! - Require stricter, role-sensitive checks for high-risk actions such as
//!   granting privileged roles or rotating administrative control.
//! - Default role escalation checks to **direct token balance only** so
//!   delegated voting power cannot silently bootstrap privileged access.

use soroban_sdk::{contracttype, token, Address, Env};

// ════════════════════════════════════════════════════════════════════
//  Storage Types
// ════════════════════════════════════════════════════════════════════

/// Storage keys for governance state.
#[contracttype]
#[derive(Clone)]
pub enum GovernanceKey {
    /// Governance token contract address.
    GovernanceToken,
    /// Minimum token balance required for ordinary governance actions.
    GovernanceThreshold,
    /// Delegated voting power: delegator -> delegate.
    Delegation(Address),
    /// Balance snapshot delegated at the time a delegation was recorded.
    DelegationAmount(Address),
    /// Total voting power delegated to an address.
    DelegatedPower(Address),
    /// Governance enabled flag.
    GovernanceEnabled,
    /// Minimum voting power required for privileged role escalation actions.
    RoleEscalationThreshold,
    /// Whether delegated voting power counts toward role escalation checks.
    RoleEscalationUseDelegatedPower,
}

/// Governance configuration for ordinary threshold-gated actions.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct GovernanceConfig {
    /// Token contract address.
    pub token: Address,
    /// Minimum token balance required for governance actions.
    pub threshold: i128,
    /// Whether governance is enabled.
    pub enabled: bool,
}

/// Additional controls for privileged role escalation paths.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RoleEscalationConfig {
    /// Minimum voting power required for role escalation.
    ///
    /// This value is always kept at or above the base governance threshold.
    pub threshold: i128,
    /// Whether delegated voting power is allowed to satisfy the escalation gate.
    ///
    /// Defaults to `false` so delegated votes cannot elevate privileged roles
    /// without an explicit opt-in by the integrating contract.
    pub allow_delegated_power: bool,
}

// ════════════════════════════════════════════════════════════════════
//  Configuration
// ════════════════════════════════════════════════════════════════════

/// Initialize governance with token and threshold.
///
/// Role escalation controls are initialized alongside the base governance
/// threshold. By default, privileged role escalation requires at least the
/// same threshold as ordinary governance and ignores delegated voting power.
///
/// # Parameters
/// - `token`: Governance token contract address.
/// - `threshold`: Minimum token balance required for governance actions.
/// - `enabled`: Whether governance is enabled from the start.
///
/// # Panics
/// - If governance is already initialized.
/// - If threshold is negative.
pub fn initialize_governance(env: &Env, token: &Address, threshold: i128, enabled: bool) {
    if env
        .storage()
        .instance()
        .has(&GovernanceKey::GovernanceToken)
    {
        panic!("governance already initialized");
    }
    assert!(threshold >= 0, "threshold must be non-negative");

    env.storage()
        .instance()
        .set(&GovernanceKey::GovernanceToken, token);
    env.storage()
        .instance()
        .set(&GovernanceKey::GovernanceThreshold, &threshold);
    env.storage()
        .instance()
        .set(&GovernanceKey::GovernanceEnabled, &enabled);
    env.storage()
        .instance()
        .set(&GovernanceKey::RoleEscalationThreshold, &threshold);
    env.storage()
        .instance()
        .set(&GovernanceKey::RoleEscalationUseDelegatedPower, &false);
}

/// Get the current governance configuration.
pub fn get_governance_config(env: &Env) -> Option<GovernanceConfig> {
    let token = env
        .storage()
        .instance()
        .get(&GovernanceKey::GovernanceToken)?;
    let threshold = env
        .storage()
        .instance()
        .get(&GovernanceKey::GovernanceThreshold)?;
    let enabled = env
        .storage()
        .instance()
        .get(&GovernanceKey::GovernanceEnabled)
        .unwrap_or(false);

    Some(GovernanceConfig {
        token,
        threshold,
        enabled,
    })
}

/// Get the role escalation configuration.
///
/// For backwards compatibility with older state layouts, missing escalation
/// keys fall back to a secure default:
/// - escalation threshold = base governance threshold
/// - delegated power disabled for escalation
pub fn get_role_escalation_config(env: &Env) -> Option<RoleEscalationConfig> {
    let governance = get_governance_config(env)?;

    let threshold = env
        .storage()
        .instance()
        .get(&GovernanceKey::RoleEscalationThreshold)
        .unwrap_or(governance.threshold);
    let allow_delegated_power = env
        .storage()
        .instance()
        .get(&GovernanceKey::RoleEscalationUseDelegatedPower)
        .unwrap_or(false);

    Some(RoleEscalationConfig {
        threshold,
        allow_delegated_power,
    })
}

/// Update the base governance threshold.
///
/// If the new base threshold exceeds the current role escalation threshold, the
/// escalation threshold is automatically raised to preserve the invariant that
/// privileged role escalation is never easier than ordinary governance.
///
/// # Panics
/// - If governance is not initialized.
/// - If threshold is negative.
pub fn set_governance_threshold(env: &Env, threshold: i128) {
    require_governance_initialized(env);
    assert!(threshold >= 0, "threshold must be non-negative");

    env.storage()
        .instance()
        .set(&GovernanceKey::GovernanceThreshold, &threshold);

    let escalation = get_role_escalation_config(env).expect("governance not initialized");
    if escalation.threshold < threshold {
        env.storage()
            .instance()
            .set(&GovernanceKey::RoleEscalationThreshold, &threshold);
    }
}

/// Enable or disable governance.
///
/// # Panics
/// - If governance is not initialized.
pub fn set_governance_enabled(env: &Env, enabled: bool) {
    require_governance_initialized(env);
    env.storage()
        .instance()
        .set(&GovernanceKey::GovernanceEnabled, &enabled);
}

/// Update the threshold required for privileged role escalation.
///
/// # Panics
/// - If governance is not initialized.
/// - If threshold is negative.
/// - If threshold is lower than the base governance threshold.
pub fn set_role_escalation_threshold(env: &Env, threshold: i128) {
    require_governance_initialized(env);
    assert!(threshold >= 0, "threshold must be non-negative");

    let governance_threshold = get_base_governance_threshold(env);
    assert!(
        threshold >= governance_threshold,
        "role escalation threshold must be >= governance threshold"
    );

    env.storage()
        .instance()
        .set(&GovernanceKey::RoleEscalationThreshold, &threshold);
}

/// Configure whether delegated voting power is allowed for role escalation.
///
/// # Panics
/// - If governance is not initialized.
pub fn set_role_escalation_use_delegated_power(env: &Env, enabled: bool) {
    require_governance_initialized(env);
    env.storage()
        .instance()
        .set(&GovernanceKey::RoleEscalationUseDelegatedPower, &enabled);
}

// ════════════════════════════════════════════════════════════════════
//  Voting Power & Delegation
// ════════════════════════════════════════════════════════════════════

/// Get the direct token balance of an address.
///
/// This is the balance-owned view and does not consider delegation state.
pub fn get_direct_voting_power(env: &Env, address: &Address) -> i128 {
    let config = match get_governance_config(env) {
        Some(c) => c,
        None => return 0,
    };

    token::Client::new(env, &config.token).balance(address)
}

/// Get the total voting power of an address.
///
/// If an address has delegated its voting power to someone else, its direct
/// balance no longer counts toward its own ordinary governance power.
///
/// # Returns
/// - retained direct balance (if not delegated away) + delegated power received
pub fn get_voting_power(env: &Env, address: &Address) -> i128 {
    let direct_balance = get_direct_voting_power(env, address);
    if direct_balance == 0 && get_governance_config(env).is_none() {
        return 0;
    }

    let retained_balance = if has_outgoing_delegation(env, address) {
        0
    } else {
        direct_balance
    };

    let delegated = env
        .storage()
        .instance()
        .get(&GovernanceKey::DelegatedPower(address.clone()))
        .unwrap_or(0i128);

    retained_balance
        .checked_add(delegated)
        .expect("voting power overflow")
}

/// Get the voting power considered for role escalation.
///
/// By default, this is the caller's direct token balance only. Delegated power
/// is ignored unless explicitly enabled in the escalation configuration.
pub fn get_role_escalation_power(env: &Env, address: &Address) -> i128 {
    let escalation = match get_role_escalation_config(env) {
        Some(config) => config,
        None => return 0,
    };

    if escalation.allow_delegated_power {
        get_voting_power(env, address)
    } else {
        get_direct_voting_power(env, address)
    }
}

/// Delegate voting power to another address.
///
/// # Parameters
/// - `delegator`: Address delegating their voting power.
/// - `delegate`: Address receiving the delegated voting power.
///
/// # Notes
/// - Delegator must authorize the transaction.
/// - Previous delegation is automatically revoked using the originally
///   snapshotted delegated amount.
/// - Self-delegation is rejected to avoid double-counting direct balance.
/// - Delegated power is snapshotted at delegation time and reconciled on
///   revoke/redelegate.
pub fn delegate_voting_power(env: &Env, delegator: &Address, delegate: &Address) {
    delegator.require_auth();
    assert!(*delegator != *delegate, "cannot delegate to self");

    let balance = get_direct_voting_power(env, delegator);
    assert!(
        get_governance_config(env).is_some(),
        "governance not initialized"
    );

    // Revoke previous delegation if it exists, using the original snapshotted amount.
    if let Some(old_delegate) = env
        .storage()
        .instance()
        .get::<GovernanceKey, Address>(&GovernanceKey::Delegation(delegator.clone()))
    {
        let delegated_amount: i128 = read_delegation_amount(env, delegator);
        let old_power: i128 = env
            .storage()
            .instance()
            .get(&GovernanceKey::DelegatedPower(old_delegate.clone()))
            .unwrap_or(0);
        env.storage().instance().set(
            &GovernanceKey::DelegatedPower(old_delegate),
            &old_power
                .checked_sub(delegated_amount)
                .expect("delegated power underflow"),
        );
    }

    env.storage()
        .instance()
        .set(&GovernanceKey::Delegation(delegator.clone()), delegate);
    env.storage().instance().set(
        &GovernanceKey::DelegationAmount(delegator.clone()),
        &balance,
    );

    let current_power: i128 = env
        .storage()
        .instance()
        .get(&GovernanceKey::DelegatedPower(delegate.clone()))
        .unwrap_or(0);
    env.storage().instance().set(
        &GovernanceKey::DelegatedPower(delegate.clone()),
        &current_power
            .checked_add(balance)
            .expect("delegated power overflow"),
    );
}

/// Revoke voting power delegation.
///
/// # Parameters
/// - `delegator`: Address revoking their delegation.
///
/// # Notes
/// - Delegator must authorize the transaction.
/// - The delegate's delegated power is reduced by the originally snapshotted
///   delegation amount, not by the delegator's current balance.
pub fn revoke_delegation(env: &Env, delegator: &Address) {
    delegator.require_auth();

    let delegate: Option<Address> = env
        .storage()
        .instance()
        .get(&GovernanceKey::Delegation(delegator.clone()));

    if let Some(delegate) = delegate {
        let delegated_amount = read_delegation_amount(env, delegator);

        let current_power: i128 = env
            .storage()
            .instance()
            .get(&GovernanceKey::DelegatedPower(delegate.clone()))
            .unwrap_or(0);
        env.storage().instance().set(
            &GovernanceKey::DelegatedPower(delegate),
            &current_power
                .checked_sub(delegated_amount)
                .expect("delegated power underflow"),
        );

        env.storage()
            .instance()
            .remove(&GovernanceKey::Delegation(delegator.clone()));
        env.storage()
            .instance()
            .remove(&GovernanceKey::DelegationAmount(delegator.clone()));
    }
}

/// Get the address that a delegator has delegated to.
pub fn get_delegate(env: &Env, delegator: &Address) -> Option<Address> {
    env.storage()
        .instance()
        .get(&GovernanceKey::Delegation(delegator.clone()))
}

// ════════════════════════════════════════════════════════════════════
//  Access Control
// ════════════════════════════════════════════════════════════════════

/// Check if an address meets the ordinary governance threshold.
///
/// # Returns
/// - `true` if governance is enabled and address has sufficient governance power.
/// - `false` otherwise.
pub fn has_governance_power(env: &Env, address: &Address) -> bool {
    let config = match get_governance_config(env) {
        Some(c) => c,
        None => return false,
    };

    if !config.enabled {
        return false;
    }

    get_voting_power(env, address) >= config.threshold
}

/// Require that an address meets the ordinary governance threshold.
///
/// This helper remains fail-open when governance is uninitialized or disabled
/// so integrating contracts can keep backward-compatible behavior for ordinary
/// operations while selectively opting into stricter role escalation checks.
///
/// # Panics
/// - If governance is enabled and address does not have sufficient voting power.
pub fn require_governance_threshold(env: &Env, address: &Address) {
    address.require_auth();

    let config = match get_governance_config(env) {
        Some(c) => c,
        None => return,
    };

    if !config.enabled {
        return;
    }

    let voting_power = get_voting_power(env, address);
    assert!(
        voting_power >= config.threshold,
        "insufficient governance voting power: {} < {}",
        voting_power,
        config.threshold
    );
}

/// Check if an address meets the stricter role escalation threshold.
///
/// This helper is fail-closed: uninitialized or disabled governance returns
/// `false` for privileged role escalation checks.
pub fn has_role_escalation_power(env: &Env, address: &Address) -> bool {
    let governance = match get_governance_config(env) {
        Some(config) => config,
        None => return false,
    };
    if !governance.enabled {
        return false;
    }

    let escalation = get_role_escalation_config(env).expect("governance not initialized");
    get_role_escalation_power(env, address) >= escalation.threshold
}

/// Require that an address meets the stricter role escalation threshold.
///
/// # Panics
/// - If governance is not initialized.
/// - If governance is disabled.
/// - If address lacks sufficient role escalation voting power.
pub fn require_role_escalation_threshold(env: &Env, address: &Address) {
    address.require_auth();

    let governance = get_governance_config(env).expect("governance not initialized");
    assert!(governance.enabled, "governance disabled");

    let escalation = get_role_escalation_config(env).expect("governance not initialized");
    let power = get_role_escalation_power(env, address);
    assert!(
        power >= escalation.threshold,
        "insufficient role escalation voting power: {} < {}",
        power,
        escalation.threshold
    );
}

/// Check if governance is initialized and enabled.
pub fn is_governance_enabled(env: &Env) -> bool {
    get_governance_config(env)
        .map(|config| config.enabled)
        .unwrap_or(false)
}

// ════════════════════════════════════════════════════════════════════
//  Internal helpers
// ════════════════════════════════════════════════════════════════════

fn require_governance_initialized(env: &Env) {
    assert!(
        env.storage()
            .instance()
            .has(&GovernanceKey::GovernanceToken),
        "governance not initialized"
    );
}

fn get_base_governance_threshold(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&GovernanceKey::GovernanceThreshold)
        .expect("governance not initialized")
}

fn has_outgoing_delegation(env: &Env, address: &Address) -> bool {
    env.storage()
        .instance()
        .has(&GovernanceKey::Delegation(address.clone()))
}

fn read_delegation_amount(env: &Env, delegator: &Address) -> i128 {
    env.storage()
        .instance()
        .get(&GovernanceKey::DelegationAmount(delegator.clone()))
        .unwrap_or_else(|| get_direct_voting_power(env, delegator))
}
