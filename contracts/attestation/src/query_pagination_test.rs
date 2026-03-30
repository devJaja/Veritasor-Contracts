#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};

fn period_str(env: &Env, i: u32) -> String {
    let s = match i {
        1 => "2026-01",
        2 => "2026-02",
        3 => "2026-03",
        4 => "2026-04",
        5 => "2026-05",
        6 => "2026-06",
        7 => "2026-07",
        8 => "2026-08",
        9 => "2026-09",
        10 => "2026-10",
        11 => "2026-11",
        12 => "2026-12",
        _ => "2026-01",
    };
    String::from_str(env, s)
}

fn setup_with_attestations(
    env: &Env,
    n: u32,
) -> (Address, AttestationContractClient<'_>, Vec<String>) {
    env.mock_all_auths();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(env, &contract_id);
    client.initialize(&Address::generate(env), &0u64);
    let business = Address::generate(env);
    let mut periods = Vec::new(env);
    for i in 1..=n {
        let period = period_str(env, i);
        periods.push_back(period.clone());
        let root = BytesN::from_array(env, &[(i as u8) & 0xff; 32]);
        client.submit_attestation(&business, &period, &root, &1700000000u64, &i, &None, &None);
    }
    (business, client, periods)
}

fn setup_sparse_gaps(
    env: &Env,
) -> (Address, AttestationContractClient<'_>, Vec<String>) {
    env.mock_all_auths();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(env, &contract_id);
    client.initialize(&Address::generate(env));
    let business = Address::generate(env);
    let mut periods = Vec::new(env);
    let total_periods: u32 = 100;
    for i in 1..=total_periods {
        let period = period_str(env, ((i - 1) % 12) + 1); // Cycle periods
        periods.push_back(period.clone());
        // Submit only every 10th period (indices 10,20,30,...,100)
        if i % 10 == 0 {
            let root = BytesN::from_array(env, &[(i as u8); 32]);
            let ver = i / 10; // Version based on hit index
            client.submit_attestation(&business, &period, &root, &1700000000u64, &ver, &None);
        }
    }
    (business, client, periods)
}

fn setup_sparse_clustered(
    env: &Env,
) -> (Address, AttestationContractClient<'_>, Vec<String>) {
    env.mock_all_auths();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(env, &contract_id);
    client.initialize(&Address::generate(env));
    let business = Address::generate(env);
    let mut periods = Vec::new(env);
    // Dense cluster 1-10
    for i in 1..=10 {
        let period = period_str(env, i);
        periods.push_back(period.clone());
        let root = BytesN::from_array(env, &[i as u8; 32]);
        client.submit_attestation(&business, &period, &root, &1700000000u64, &i, &None);
    }
    // Sparse gap 11-200 (no attestations)
    for i in 11..=200 {
        let period = format!("20{}01", i / 10); // Dummy sparse periods
        periods.push_back(String::from_str(env, &period));
    }
    (business, client, periods)
}

#[test]
fn get_attestations_page_empty_periods() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 3);
    let (out, next) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &10,
        &0,
    );
    assert_eq!(out.len(), 3);
    assert_eq!(next, 3);
}

#[test]
fn get_attestations_page_cursor_past_end() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 2);
    let (out, next) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &10,
        &10,
    );
    assert_eq!(out.len(), 0);
    assert_eq!(next, 10);
}

#[test]
fn get_attestations_page_limit_caps_results() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 10);
    let (out, next) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &3,
        &0,
    );
    assert_eq!(out.len(), 3);
    assert_eq!(next, 3);
}

#[test]
fn get_attestations_page_second_page() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 5);
    let (page1, next1) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &2,
        &0,
    );
    assert_eq!(page1.len(), 2);
    assert_eq!(next1, 2);
    let (page2, next2) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &2,
        &next1,
    );
    assert_eq!(page2.len(), 2);
    assert_eq!(next2, 4);
    let (page3, next3) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &2,
        &next2,
    );
    assert_eq!(page3.len(), 1);
    assert_eq!(next3, 5);
}

#[test]
fn get_attestations_page_round_trip_all_pages() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 12);
    let mut all: Vec<(String, BytesN<32>, u64, u32, u32)> = Vec::new(&env);
    let mut cursor = 0u32;
    loop {
        let (page, next) = client.get_attestations_page(
            &business,
            &periods,
            &None,
            &None,
            &STATUS_FILTER_ALL,
            &None,
            &5,
            &cursor,
        );
        for i in 0..page.len() {
            all.push_back(page.get(i).unwrap());
        }
        if next >= periods.len() {
            break;
        }
        cursor = next;
    }
    assert_eq!(all.len(), 12);
    for i in 0..12u32 {
        let (period, _root, _ts, ver, status) = all.get(i).unwrap();
        assert_eq!(period, period_str(&env, i + 1));
        assert_eq!(ver, i + 1);
        assert_eq!(status, STATUS_ACTIVE);
    }
}

#[test]
fn get_attestations_page_filter_period_range() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 5);
    let start = Some(String::from_str(&env, "2026-02"));
    let end = Some(String::from_str(&env, "2026-04"));
    let (out, next) = client.get_attestations_page(
        &business,
        &periods,
        &start,
        &end,
        &STATUS_FILTER_ALL,
        &None,
        &10,
        &0,
    );
    assert_eq!(out.len(), 3);
    assert_eq!(out.get(0).unwrap().0, String::from_str(&env, "2026-02"));
    assert_eq!(out.get(1).unwrap().0, String::from_str(&env, "2026-03"));
    assert_eq!(out.get(2).unwrap().0, String::from_str(&env, "2026-04"));
    assert_eq!(next, 5);
}

#[test]
fn get_attestations_page_filter_version() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 5);
    let (out, _) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &Some(3),
        &10,
        &0,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out.get(0).unwrap().3, 3);
}

#[test]
fn get_attestations_page_filter_active_after_revoke() {
    let env = Env::default();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin, &0u64);
    let business = Address::generate(&env);
    let mut periods = Vec::new(&env);
    for i in 1..=3 {
        let period = period_str(&env, i);
        periods.push_back(period.clone());
        let root = BytesN::from_array(&env, &[(i as u8) & 0xff; 32]);
        client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &None);
    }
    let revoke_period = String::from_str(&env, "2026-02");
    client.revoke_attestation(&admin, &business, &revoke_period, &soroban_sdk::String::from_str(&env, "test reason"), &0u64);
    let (active_only, _) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_ACTIVE,
        &None,
        &10,
        &0,
    );
    assert_eq!(active_only.len(), 2);
    let (revoked_only, _) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_REVOKED,
        &None,
        &10,
        &0,
    );
    assert_eq!(revoked_only.len(), 1);
    assert_eq!(
        revoked_only.get(0).unwrap().0,
        String::from_str(&env, "2026-02")
    );
}

#[test]
fn get_attestations_page_limit_capped_at_max() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 12);
    let (out, _) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &100,
        &0,
    );
    assert!(out.len() <= 30);
    assert_eq!(out.len(), 12);
}

#[test]
fn get_attestations_page_empty_result_when_no_match() {
    let env = Env::default();
    let (business, client, periods) = setup_with_attestations(&env, 2);
    let start = Some(String::from_str(&env, "2027-01"));
    let end = Some(String::from_str(&env, "2027-12"));
    let (out, next) = client.get_attestations_page(
        &business,
        &periods,
        &start,
        &end,
        &STATUS_FILTER_ALL,
        &None,
        &10,
        &0,
    );
    assert_eq!(out.len(), 0);
    assert_eq!(next, 2);
}

#[test]
fn init_and_revoke_attestation() {
    let env = Env::default();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin, &0u64);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &None);
    client.revoke_attestation(&admin, &business, &period, &soroban_sdk::String::from_str(&env, "test reason"), &0u64);
    let mut periods = Vec::new(&env);
    periods.push_back(period.clone());
    let (out, _) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_REVOKED,
        &None,
        &10,
        &0,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out.get(0).unwrap().4, STATUS_REVOKED);
}

#[test]
fn revoke_attestation_non_admin_panics() {
    let env = Env::default();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin, &0u64);
    let other = Address::generate(&env);
    let business = Address::generate(&env);
    let period = String::from_str(&env, "2026-02");
    let root = BytesN::from_array(&env, &[1u8; 32]);
    client.submit_attestation(&business, &period, &root, &1700000000u64, &1u32, &None, &None);
    let result = client.try_revoke_attestation(&other, &business, &period, &soroban_sdk::String::from_str(&env, "test reason"), &0u64);
    assert!(result.is_err());
}

#[test]
fn periods_list_includes_missing_attestations_skipped() {
    let env = Env::default();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    env.mock_all_auths();
    client.initialize(&Address::generate(&env), &0u64);
    let business = Address::generate(&env);
    let p1 = String::from_str(&env, "2026-01");
    let p2 = String::from_str(&env, "2026-02");
    let p3 = String::from_str(&env, "2026-03");
    client.submit_attestation(
        &business,
        &p1,
        &BytesN::from_array(&env, &[1u8; 32]),
        &1700000000u64,
        &1u32,
        &None,
        &None,
    );
    client.submit_attestation(
        &business,
        &p3,
        &BytesN::from_array(&env, &[3u8; 32]),
        &1700000000u64,
        &1u32,
        &None,
        &None,
    );
    let mut periods = Vec::new(&env);
    periods.push_back(p1.clone());
    periods.push_back(p2.clone());
    periods.push_back(p3.clone());
    let (out, next) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &10,
        &0,
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out.get(0).unwrap().0, p1);
    assert_eq!(out.get(1).unwrap().0, p3);
    assert_eq!(next, 3);
}

#[test]
/// Tests pagination stability over large gaps in attestation data - every 10th period has attestation in 100 periods.
/// Verifies correct number of hits (10), next_cursor advances over all gaps correctly.
fn test_sparse_gaps_large_skip() {
    let env = Env::default();
    let (business, client, periods) = setup_sparse_gaps(&env);
    let (out, next) = client.get_attestations_page(
        &business,
        &periods,
        &None,
        &None,
        &STATUS_FILTER_ALL,
        &None,
        &20, // Larger than hits
        &0,
    );
    assert_eq!(out.len(), 10); // 10 hits
    assert_eq!(next, 100); // Scanned all
    // Verify roots match expected sparse positions
    for i in 0..10u32 {
        let expected_root = BytesN::from_array(&env, &[( (i+1)*10 as u8); 32]);
        assert_eq!(out.get(i as usize).unwrap().1, expected_root);
    }
}

#[test]
/// Tests clustered attestations followed by large sparse gap.
/// Roundtrip full to ensure skips large gap without full iteration issues.
fn test_sparse_clustered_then_gap() {
    let env = Env::default();
    let (business, client, periods) = setup_sparse_clustered(&env);
    // First page should get all dense
    let (page1, next1) = client.get_attestations_page(&business, &periods, &None, &None, &STATUS_FILTER_ALL, &None, &15, &0);
    assert_eq!(page1.len(), 10);
    assert_eq!(next1, 200); // Scans all (dense 10 + gap 190)
    // No more pages
    let (page2, next2) = client.get_attestations_page(&business, &periods, &None, &None, &STATUS_FILTER_ALL, &None, &15, &next1);
    assert_eq!(page2.len(), 0);
}

#[test]
/// Test adversarial unsorted periods list with sparse hits.
fn test_sparse_adversarial_unsorted_periods() {
    let env = Env::default();
    let (business, client, mut periods) = setup_sparse_gaps(&env);
    // Shuffle periods (in test env, simulate by reversing)
    let len = periods.len();
    for i in 0..len/2 {
        let temp = periods.get(i).unwrap().clone();
        periods.set(i, periods.get(len as usize - 1 - i as usize).unwrap().clone());
        periods.set(len as usize - 1 - i as usize, temp);
    }
    let (out, next) = client.get_attestations_page(&business, &periods, &None, &None, &STATUS_FILTER_ALL, &None, &20, &0);
    assert_eq!(out.len(), 10); // Still finds all
    assert_eq!(next, 100);
}

#[test]
/// Duplicate periods in list - ensure stable handling.
fn test_sparse_duplicates_in_periods() {
    let env = Env::default();
    let contract_id = env.register(AttestationContract, ());
    let client = AttestationContractClient::new(&env, &contract_id);
    env.mock_all_auths();
    let business = Address::generate(&env);
    let p1 = String::from_str(&env, "2026-01");
    client.submit_attestation(&business, &p1, &BytesN::from_array(&env, &[1;32]), &1700000000u64, &1, &None);
    let mut periods = Vec::new(&env);
    periods.push_back(p1.clone());
    periods.push_back(p1.clone()); // Duplicate
    periods.push_back(String::from_str(&env, "2026-02")); // Missing
    let (out, next) = client.get_attestations_page(&business, &periods, &None, &None, &STATUS_FILTER_ALL, &None, &10, &0);
    // Expect 1 hit (dupe loads same)
    assert_eq!(out.len(), 1);
    assert_eq!(next, 3);
}

#[test]
/// Jump cursor into middle of sparse gap.
fn test_sparse_cursor_jump_gap() {
    let env = Env::default();
    let (business, client, periods) = setup_sparse_gaps(&env);
    // Start after first hit (cursor 10), expect 9 hits, scan to end
    let (out, next) = client.get_attestations_page(&business, &periods, &None, &None, &STATUS_FILTER_ALL, &None, &20, &10);
    assert_eq!(out.len(), 9);
    assert_eq!(next, 100);
}

#[test]
/// 100% sparse - all periods missing attestations.
fn test_sparse_100_percent_empty() {
    let env = Env::default();
    let (business, client, periods) = setup_sparse_gaps(&env);
    // Revoke all to make empty (or setup no submits)
    // For simplicity, use periods without submits, but since helper has some, filter version impossible
    let mut empty_periods = Vec::new(&env);
    for i in 1..=100u32 {
        empty_periods.push_back(period_str(&env, ((i-1)%12)+1));
    }
    let contract_id = env.register(AttestationContract, ());
    let empty_client = AttestationContractClient::new(&env, &contract_id);
    empty_client.initialize(&Address::generate(&env));
    let (out, next) = empty_client.get_attestations_page(&business, &empty_periods, &None, &None, &STATUS_FILTER_ALL, &None, &10, &0);
    assert_eq!(out.len(), 0);
    assert_eq!(next, 10);
    // Second page
    let (out2, next2) = empty_client.get_attestations_page(&business, &empty_periods, &None, &None, &STATUS_FILTER_ALL, &None, &10, &next);
    assert_eq!(out2.len(), 0);
    assert_eq!(next2, 20);
}

#[test]
/// Near u32 max cursor with sparse end.
fn test_sparse_max_cursor_u32() {
    let env = Env::default();
    let (business, client, periods) = setup_sparse_gaps(&env);
    let large_cursor = u32::MAX - 50; // Near end
    let (out, next) = client.get_attestations_page(&business, &periods, &None, &None, &STATUS_FILTER_ALL, &None, &10, &large_cursor);
    // Since periods.len()==100, if large_cursor >=100, empty
    if large_cursor >= periods.len() {
        assert_eq!(out.len(), 0);
        assert_eq!(next, large_cursor); // or min(large + limit, len)?
    }
}

#[test]
/// Filters interact correctly with sparse gaps.
fn test_sparse_filter_interaction() {
    let env = Env::default();
    let (business, client, periods) = setup_sparse_gaps(&env);
    // Filter to version 5 (only one hit)
    let (out, next) = client.get_attestations_page(&business, &periods, &None, &None, &STATUS_FILTER_ALL, &Some(5), &20, &0);
    assert_eq!(out.len(), 1);
    assert_eq!(out.get(0).unwrap().3, 5u32); // version
    assert_eq!(next, 100); // Scanned all to find it
}

#[test]
/// Repeated roundtrip on sparse data is stable.
fn test_sparse_roundtrip_stability() {
    let env1 = Env::default();
    let env2 = Env::default();
    let (business1, client1, periods1) = setup_sparse_gaps(&env1);
    let (business2, client2, periods2) = setup_sparse_gaps(&env2);
    // Roundtrip1
    let mut all1: Vec<(String, BytesN<32>, u64, u32, u32)> = Vec::new(&env1);
    let mut cursor = 0u32;
    while cursor < periods1.len() {
        let (page, next) = client1.get_attestations_page(&business1, &periods1, &None, &None, &STATUS_FILTER_ALL, &None, &5, &cursor);
        for item in page.iter() {
            all1.push_back(item.unwrap());
        }
        cursor = next;
        if page.is_empty() { break; }
    }
    // Roundtrip2
    let mut all2: Vec<(String, BytesN<32>, u64, u32, u32)> = Vec::new(&env2);
    cursor = 0u32;
    while cursor < periods2.len() {
        let (page, next) = client2.get_attestations_page(&business2, &periods2, &None, &None, &STATUS_FILTER_ALL, &None, &5, &cursor);
        for item in page.iter() {
            all2.push_back(item.unwrap());
        }
        cursor = next;
        if page.is_empty() { break; }
    }
    assert_eq!(all1.len(), all2.len());
    for i in 0..all1.len() {
        assert_eq!(all1.get(i).unwrap(), all2.get(i).unwrap());
    }
}
