#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, String, Vec};

/// Attestor staking client: WASM import for wasm32, crate client for host builds.
#[cfg(target_arch = "wasm32")]
mod attestor_staking_import {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/veritasor_attestor_staking.wasm"
    );
    pub use Client as AttestorStakingContractClient;
}

const STATUS_KEY_TAG: u32 = 1;
const ADMIN_KEY_TAG: (u32,) = (2,);
const ANOMALY_KEY_TAG: (u32,) = (3,);
const AUTHORIZED_KEY_TAG: (u32,) = (4,);
const ANOMALY_SCORE_MAX: u32 = 100;
const NONCE_CHANNEL_BUSINESS: u32 = 1;

pub const STATUS_ACTIVE: u32 = 0;
pub const STATUS_REVOKED: u32 = 1;

// Type aliases to reduce complexity
pub type AttestationData = (BytesN<32>, u64, u32, i128, Option<BytesN<32>>, Option<u64>);
pub type RevocationData = (Address, u64, String);
pub type AttestationWithRevocation = (AttestationData, Option<RevocationData>);
pub type AttestationStatusResult = Vec<(String, Option<AttestationData>, Option<RevocationData>)>;

// Feature modules
pub mod access_control;
pub mod dynamic_fees;
pub mod events;
pub mod fees;
pub mod multisig;
pub mod rate_limit;
pub mod registry;
pub mod dispute;
pub mod extended_metadata;

pub use access_control::{ROLE_ADMIN, ROLE_ATTESTOR, ROLE_BUSINESS, ROLE_OPERATOR};
pub use dynamic_fees::{DataKey, FeeConfig};
pub use fees::FlatFeeConfig;
pub use multisig::{Proposal, ProposalAction, ProposalStatus};
pub use rate_limit::RateLimitConfig;
pub use registry::{BusinessRecord, BusinessStatus};
pub use dispute::{Dispute, DisputeOutcome, DisputeStatus, DisputeType, OptionalResolution, DisputeResolution};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationRange {
    pub start_period: u32,
    pub end_period: u32,
    pub merkle_root: BytesN<32>,
    pub timestamp: u64,
    pub version: u32,
    pub fee_paid: i128,
    pub revoked: bool,
}

#[contracttype]
pub enum MultiPeriodKey {
    Ranges(Address),
}

#[contracttype]
pub struct BatchAttestationItem {
    pub business: Address,
    pub period: String,
    pub merkle_root: BytesN<32>,
    pub timestamp: u64,
    pub version: u32,
    pub expiry_timestamp: Option<u64>,
}

#[contract]
pub struct AttestationContract;

#[contractimpl]
impl AttestationContract {
    pub fn initialize(env: Env, admin: Address, _nonce: u64) {
        if dynamic_fees::is_initialized(&env) {
            panic!("already initialized");
        }
        admin.require_auth();
        dynamic_fees::set_admin(&env, &admin);
        access_control::grant_role(&env, &admin, ROLE_ADMIN);
    }

    pub fn configure_fees(env: Env, token: Address, collector: Address, base_fee: i128, enabled: bool) {
        dynamic_fees::require_admin(&env);
        let config = FeeConfig { token, collector, base_fee, enabled };
        dynamic_fees::set_fee_config(&env, &config);
    }

    pub fn get_admin(env: Env) -> Address {
        dynamic_fees::get_admin(&env)
    }

    pub fn grant_role(env: Env, caller: Address, account: Address, role: u32, _nonce: u64) {
        access_control::require_admin(&env, &caller);
        access_control::grant_role(&env, &account, role);
    }

    pub fn has_role(env: Env, account: Address, role: u32) -> bool {
        access_control::has_role(&env, &account, role)
    }

    pub fn submit_attestation(
        env: Env,
        business: Address,
        period: String,
        merkle_root: BytesN<32>,
        timestamp: u64,
        version: u32,
        proof_hash: Option<BytesN<32>>,
        expiry_timestamp: Option<u64>,
    ) {
        business.require_auth();
        let key = DataKey::Attestation(business.clone(), period.clone());
        if env.storage().instance().has(&key) {
            panic!("attestation exists");
        }

        let fee = dynamic_fees::collect_fee(&env, &business);
        dynamic_fees::increment_business_count(&env, &business);

        let data = (
            merkle_root.clone(),
            timestamp,
            version,
            fee,
            proof_hash.clone(),
            expiry_timestamp,
        );
        env.storage().instance().set(&key, &data);
        
        events::emit_attestation_submitted(
            &env,
            &business,
            &period,
            &merkle_root,
            timestamp,
            version,
            fee,
            &proof_hash,
            expiry_timestamp,
        );
    }

    pub fn get_attestation(
        env: Env,
        business: Address,
        period: String,
    ) -> Option<AttestationData> {
        let key = DataKey::Attestation(business, period);
        env.storage().instance().get(&key)
    }

    pub fn is_expired(env: Env, business: Address, period: String) -> bool {
        if let Some((_, _, _, _, _, Some(expiry_ts))) = Self::get_attestation(env.clone(), business, period) {
            env.ledger().timestamp() >= expiry_ts
        } else {
            false
        }
    }

    pub fn is_revoked(env: Env, business: Address, period: String) -> bool {
        dispute::is_attestation_revoked(&env, &business, &period)
    }

    /// Returns revocation metadata for an attestation, if it has been revoked.
    pub fn get_revocation_info(
        env: Env,
        business: Address,
        period: String,
    ) -> Option<RevocationData> {
        dispute::get_attestation_revocation(&env, &business, &period)
    }

    /// Returns attestation data together with optional revocation metadata.
    pub fn get_attestation_with_status(
        env: Env,
        business: Address,
        period: String,
    ) -> Option<AttestationWithRevocation> {
        let attestation = Self::get_attestation(env.clone(), business.clone(), period.clone())?;
        let revocation = Self::get_revocation_info(env, business, period);
        Some((attestation, revocation))
    }

    /// Verifies an attestation against the expected root and revocation status.
    pub fn verify_attestation(
        env: Env,
        business: Address,
        period: String,
        merkle_root: BytesN<32>,
    ) -> bool {
        match Self::get_attestation(env.clone(), business.clone(), period.clone()) {
            Some((stored_root, _, _, _, _, _)) => {
                stored_root == merkle_root && !Self::is_revoked(env, business, period)
            }
            None => false,
        }
    }

    /// Batch-queries attestation and revocation state for the requested periods.
    pub fn get_business_attestations(
        env: Env,
        business: Address,
        periods: Vec<String>,
    ) -> AttestationStatusResult {
        let mut results = Vec::new(&env);
        for period in periods.iter() {
            let attestation = Self::get_attestation(env.clone(), business.clone(), period.clone());
            let revocation = Self::get_revocation_info(env.clone(), business.clone(), period.clone());
            results.push_back((period, attestation, revocation));
        }
        results
    }

    pub fn revoke_attestation(
        env: Env,
        caller: Address,
        business: Address,
        period: String,
        reason: String,
        _nonce: u64,
    ) {
        dispute::require_revocation_authorized(&env, &caller, &business, &period);
        let revoked_at = env.ledger().timestamp();
        let revocation = (caller.clone(), revoked_at, reason.clone());
        dispute::store_attestation_revocation(&env, &business, &period, &revocation);
        events::emit_attestation_revoked(&env, &business, &period, &caller, &reason);
    }

    pub fn migrate_attestation(
        env: Env,
        caller: Address,
        business: Address,
        period: String,
        new_merkle_root: BytesN<32>,
        new_version: u32,
    ) {
        access_control::require_admin(&env, &caller);
        dispute::require_not_revoked_for_update(&env, &business, &period);
        let key = DataKey::Attestation(business.clone(), period.clone());
        let (old_root, ts, old_ver, fee, proof_hash, expiry): AttestationData = env
            .storage()
            .instance()
            .get(&key)
            .expect("not found");

        if new_version <= old_ver {
            panic!("version too low");
        }

        let data = (new_merkle_root.clone(), ts, new_version, fee, proof_hash, expiry);
        env.storage().instance().set(&key, &data);
        events::emit_attestation_migrated(
            &env,
            &business,
            &period,
            &old_root,
            &new_merkle_root,
            old_ver,
            new_version,
            &caller,
        );
    }

    /// Pauses state-changing administrative flows.
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        let caller_is_admin = caller == dynamic_fees::get_admin(&env)
            || access_control::has_role(&env, &caller, ROLE_ADMIN)
            || access_control::has_role(&env, &caller, ROLE_OPERATOR);
        assert!(caller_is_admin, "caller must have ADMIN or OPERATOR role");
        access_control::set_paused(&env, true);
        events::emit_paused(&env, &caller);
    }

    /// Restores state-changing administrative flows.
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        let caller_is_admin = caller == dynamic_fees::get_admin(&env)
            || access_control::has_role(&env, &caller, ROLE_ADMIN)
            || access_control::has_role(&env, &caller, ROLE_OPERATOR);
        assert!(caller_is_admin, "caller must have ADMIN or OPERATOR role");
        access_control::set_paused(&env, false);
        events::emit_unpaused(&env, &caller);
    }

    pub fn submit_multi_period_attestation(
        env: Env,
        business: Address,
        start_period: u32,
        end_period: u32,
        merkle_root: BytesN<32>,
        timestamp: u64,
        version: u32,
    ) {
        business.require_auth();
        let key = MultiPeriodKey::Ranges(business.clone());
        let mut ranges: Vec<AttestationRange> = env.storage().instance().get(&key).unwrap_or(Vec::new(&env));

        for range in ranges.iter() {
            if !range.revoked && start_period <= range.end_period && end_period >= range.start_period {
                panic!("overlap");
            }
        }

        let fee = dynamic_fees::collect_fee(&env, &business);
        dynamic_fees::increment_business_count(&env, &business);

        ranges.push_back(AttestationRange {
            start_period,
            end_period,
            merkle_root: merkle_root.clone(),
            timestamp,
            version,
            fee_paid: fee,
            revoked: false,
        });
        env.storage().instance().set(&key, &ranges);
    }

    pub fn configure_key_rotation(
        env: Env,
        timelock_ledgers: u32,
        confirmation_window_ledgers: u32,
        cooldown_ledgers: u32,
    ) {
        dynamic_fees::require_admin(&env);
        let config = veritasor_common::key_rotation::RotationConfig {
            timelock_ledgers,
            confirmation_window_ledgers,
            cooldown_ledgers,
        };
        veritasor_common::key_rotation::set_rotation_config(&env, &config);
    }

    pub fn propose_key_rotation(env: Env, new_admin: Address) {
        let current_admin = dynamic_fees::require_admin(&env);
        veritasor_common::key_rotation::propose_rotation(&env, &current_admin, &new_admin);
    }

    pub fn confirm_key_rotation(env: Env, caller: Address) {
        let old_admin = dynamic_fees::get_admin(&env);
        let pending = veritasor_common::key_rotation::get_pending_rotation(&env).expect("no pending");
        let new_admin = pending.new_admin;

        caller.require_auth();
        assert!(caller == new_admin, "not authorized");

        veritasor_common::key_rotation::confirm_rotation(&env, &new_admin);
        dynamic_fees::set_admin(&env, &new_admin);
        access_control::revoke_role(&env, &old_admin, ROLE_ADMIN);
        access_control::grant_role(&env, &new_admin, ROLE_ADMIN);
    }

    pub fn open_dispute(
        env: Env,
        challenger: Address,
        business: Address,
        period: String,
        dispute_type: DisputeType,
        evidence: String,
    ) -> u64 {
        challenger.require_auth();
        let id = dispute::generate_dispute_id(&env);
        let d = Dispute {
            id,
            challenger: challenger.clone(),
            business,
            period,
            status: DisputeStatus::Open,
            dispute_type,
            evidence,
            timestamp: env.ledger().timestamp(),
            resolution: OptionalResolution::None,
        };
        dispute::store_dispute(&env, &d);
        id
    }

    pub fn get_dispute(env: Env, id: u64) -> Option<Dispute> {
        dispute::get_dispute(&env, id)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod revocation_test;
