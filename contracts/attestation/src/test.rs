#![cfg(test)]

use super::{AttestationContract, AttestationContractClient, AttestationStatusResult, AttestationWithRevocation};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, String, Vec};

pub struct TestEnv {
    pub env: Env,
    pub client: AttestationContractClient<'static>,
    pub admin: Address,
}

impl TestEnv {
    pub fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(AttestationContract, ());
        let client = AttestationContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin, &0u64);

        Self { env, client, admin }
    }

    pub fn submit_attestation(
        &self,
        business: Address,
        period: String,
        merkle_root: BytesN<32>,
        timestamp: u64,
        version: u32,
    ) {
        self.client.submit_attestation(
            &business,
            &period,
            &merkle_root,
            &timestamp,
            &version,
            &None,
            &None,
        );
    }

    pub fn revoke_attestation(
        &self,
        caller: Address,
        business: Address,
        period: String,
        reason: String,
    ) {
        self.client
            .revoke_attestation(&caller, &business, &period, &reason, &0u64);
    }

    pub fn migrate_attestation(
        &self,
        caller: Address,
        business: Address,
        period: String,
        new_merkle_root: BytesN<32>,
        new_version: u32,
    ) {
        self.client.migrate_attestation(
            &caller,
            &business,
            &period,
            &new_merkle_root,
            &new_version,
        );
    }

    pub fn is_revoked(&self, business: Address, period: String) -> bool {
        self.client.is_revoked(&business, &period)
    }

    pub fn get_revocation_info(
        &self,
        business: Address,
        period: String,
    ) -> Option<(Address, u64, String)> {
        self.client.get_revocation_info(&business, &period)
    }

    pub fn get_attestation(
        &self,
        business: Address,
        period: String,
    ) -> Option<(BytesN<32>, u64, u32, i128, Option<BytesN<32>>, Option<u64>)> {
        self.client.get_attestation(&business, &period)
    }

    pub fn get_attestation_with_status(
        &self,
        business: Address,
        period: String,
    ) -> Option<AttestationWithRevocation> {
        self.client.get_attestation_with_status(&business, &period)
    }

    pub fn verify_attestation(
        &self,
        business: Address,
        period: String,
        merkle_root: BytesN<32>,
    ) -> bool {
        self.client.verify_attestation(&business, &period, &merkle_root)
    }

    pub fn get_business_attestations(
        &self,
        business: Address,
        periods: Vec<String>,
    ) -> AttestationStatusResult {
        self.client.get_business_attestations(&business, &periods)
    }

    pub fn pause(&self, caller: Address) {
        self.client.pause(&caller);
    }
}