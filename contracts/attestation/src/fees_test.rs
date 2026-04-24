#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{Address, BytesN, Env, String};

struct TestSetup<'a> {
    env: Env,
    client: AttestationContractClient<'a>,
    admin: Address,
    token_addr: Address,
    collector: Address,
}

fn setup_with_flat_fees(amount: i128) -> TestSetup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let collector = Address::generate(&env);

    // Deploy a Stellar asset token for fee payment.
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = token_contract.address().clone();

    // Register and initialize the attestation contract.
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    client.initialize(&admin, &0);

    // Configure flat fees.
    client.configure_flat_fee(&token_addr, &collector, &amount, &true);

    TestSetup {
        env,
        client,
        admin,
        token_addr,
        collector,
    }
}

fn mint(env: &Env, token_addr: &Address, to: &Address, amount: i128) {
    let stellar = StellarAssetClient::new(env, token_addr);
    stellar.mint(to, &amount);
}

fn balance(env: &Env, token_addr: &Address, who: &Address) -> i128 {
    let token = TokenClient::new(env, token_addr);
    token.balance(who)
}

#[test]
fn test_collect_flat_fee_success() {
    let t = setup_with_flat_fees(500);
    let business = Address::generate(&t.env);
    mint(&t.env, &t.token_addr, &business, 1000);

    let period = String::from_str(&t.env, "2026-02");
    let root = BytesN::from_array(&t.env, &[1u8; 32]);
    
    t.client.submit_attestation(&business, &period, &root, &1_700_000_000, &1, &None, &None, &0);

    assert_eq!(balance(&t.env, &t.token_addr, &business), 500);
    assert_eq!(balance(&t.env, &t.token_addr, &t.collector), 500);

    let (fee_paid, _, _, _, _, _) = match t.client.get_attestation(&business, &period) {
        Some((_, _, _, fee, _, _)) => (fee, 0, 0, 0, 0, 0), // Simplifying for the test
        None => panic!("attestation not found"),
    };
    // Wait, let's fix the tuple unpack properly based on lib.rs:655
    let record = t.client.get_attestation(&business, &period).unwrap();
    assert_eq!(record.3, 500); // fee_paid is the 4th element (index 3)
}

#[test]
fn test_flat_fee_disabled() {
    let t = setup_with_flat_fees(500);
    t.client.configure_flat_fee(&t.token_addr, &t.collector, &500, &false);

    let business = Address::generate(&t.env);
    let period = String::from_str(&t.env, "2026-02");
    let root = BytesN::from_array(&t.env, &[1u8; 32]);
    
    t.client.submit_attestation(&business, &period, &root, &1_700_000_000, &1, &None, &None, &0);

    assert_eq!(balance(&t.env, &t.token_addr, &t.collector), 0);
    let record = t.client.get_attestation(&business, &period).unwrap();
    assert_eq!(record.3, 0);
}

#[test]
fn test_zero_flat_fee() {
    let t = setup_with_flat_fees(0);
    let business = Address::generate(&t.env);
    let period = String::from_str(&t.env, "2026-02");
    let root = BytesN::from_array(&t.env, &[1u8; 32]);
    
    t.client.submit_attestation(&business, &period, &root, &1_700_000_000, &1, &None, &None, &0);

    assert_eq!(balance(&t.env, &t.token_addr, &t.collector), 0);
}

#[test]
#[should_panic]
fn test_flat_fee_insufficient_balance() {
    let t = setup_with_flat_fees(500);
    let business = Address::generate(&t.env);
    mint(&t.env, &t.token_addr, &business, 499); // 1 stroop short

    let period = String::from_str(&t.env, "2026-02");
    let root = BytesN::from_array(&t.env, &[1u8; 32]);
    
    t.client.submit_attestation(&business, &period, &root, &1_700_000_000, &1, &None, &None, &0);
}

#[test]
fn test_combined_fees() {
    let t = setup_with_flat_fees(500);
    // Also enable dynamic fees: base 1000
    let dyn_collector = Address::generate(&t.env);
    t.client.configure_fees(&t.token_addr, &dyn_collector, &1000, &true);

    let business = Address::generate(&t.env);
    mint(&t.env, &t.token_addr, &business, 2000);

    let period = String::from_str(&t.env, "2026-02");
    let root = BytesN::from_array(&t.env, &[1u8; 32]);
    
    t.client.submit_attestation(&business, &period, &root, &1_700_000_000, &1, &None, &None, &0);

    // Total = 500 (flat) + 1000 (dynamic) = 1500
    assert_eq!(balance(&t.env, &t.token_addr, &business), 500);
    assert_eq!(balance(&t.env, &t.token_addr, &t.collector), 500);
    assert_eq!(balance(&t.env, &t.token_addr, &dyn_collector), 1000);

    let record = t.client.get_attestation(&business, &period).unwrap();
    assert_eq!(record.3, 1500);
}

#[contract]
struct MockDao;

#[contractimpl]
impl MockDao {
    pub fn get_attestation_flat_fee_config(env: Env) -> Option<(Address, Address, i128, bool)> {
        // We use a hardcoded amount for testing the override
        let token = Address::generate(&env);
        let collector = Address::generate(&env);
        Some((token, collector, 1000, true))
    }
}

// DAO Override Test
#[test]
fn test_flat_fee_dao_override() {
    let t = setup_with_flat_fees(500);
    
    let dao_id = t.env.register(MockDao, ());
    t.client.set_flat_fee_dao(&dao_id);

    // The mock DAO returns 1000, which should override the 500 set in setup
    let config = t.client.get_effective_flat_fee_config().unwrap();
    assert_eq!(config.amount, 1000);
    
    // Original config is still 500
    let original = t.client.get_flat_fee_config().unwrap();
    assert_eq!(original.amount, 500);
}



