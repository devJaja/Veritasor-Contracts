use soroban_sdk::{Env, String};

use crate::interface_spec_check::{
    get_event_count, get_expected_events, get_expected_methods, get_expected_structs,
    get_method_count, get_struct_count, is_event_documented, is_method_documented,
    is_struct_documented, verify_interface_consistency, VerificationResult,
};

#[test]
fn test_verification_result_new() {
    let env = Env::default();
    let result = VerificationResult::new(&env);

    assert!(result.passed);
    assert_eq!(result.missing_methods.len(), 0);
    assert_eq!(result.undocumented_methods.len(), 0);
    assert_eq!(result.missing_events.len(), 0);
    assert_eq!(result.missing_structs.len(), 0);
    assert_eq!(result.errors.len(), 0);
}

#[test]
fn test_verification_result_add_missing_method() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);
    result.add_missing_method(&env, String::from_str(&env, "test_method"));
    assert!(!result.passed);
    assert_eq!(result.missing_methods.len(), 1);
}

#[test]
fn test_verification_result_add_undocumented_method() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);
    result.add_undocumented_method(&env, String::from_str(&env, "undoc_method"));
    assert!(!result.passed);
    assert_eq!(result.undocumented_methods.len(), 1);
}

#[test]
fn test_verification_result_add_missing_event() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);
    result.add_missing_event(&env, String::from_str(&env, "test_event"));
    assert!(!result.passed);
    assert_eq!(result.missing_events.len(), 1);
}

#[test]
fn test_verification_result_add_missing_struct() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);
    result.add_missing_struct(&env, String::from_str(&env, "TestStruct"));
    assert!(!result.passed);
    assert_eq!(result.missing_structs.len(), 1);
}

#[test]
fn test_verification_result_add_error() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);
    result.add_error(&env, String::from_str(&env, "test error"));
    assert!(!result.passed);
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn test_method_count() {
    let env = Env::default();
    assert_eq!(get_method_count(&env), 83);
}

#[test]
fn test_event_count() {
    let env = Env::default();
    assert_eq!(get_event_count(&env), 13);
}

#[test]
fn test_struct_count() {
    let env = Env::default();
    assert_eq!(get_struct_count(&env), 17);
}

#[test]
fn test_is_method_documented() {
    let env = Env::default();
    assert!(is_method_documented(&env, "AttestationContract", "initialize"));
    assert!(!is_method_documented(&env, "AttestationContract", "nonexistent_method"));
}

#[test]
fn test_verify_interface_consistency() {
    let env = Env::default();
    let result = verify_interface_consistency(&env);
    assert!(result.passed);
}

// --- NEW ISSUE #253 REGRESSION TESTS ---

#[test]
fn test_verification_result_add_version_mismatch() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);
    result.add_version_mismatch(&env, String::from_str(&env, "v1 != v2"));
    assert!(!result.passed);
    assert_eq!(result.version_mismatches.len(), 1);
}

#[test]
fn test_verification_result_add_cross_crate_violation() {
    let env = Env::default();
    let mut result = VerificationResult::new(&env);
    result.add_cross_crate_violation(&env, String::from_str(&env, "Signature changed"));
    assert!(!result.passed);
    assert_eq!(result.cross_crate_violations.len(), 1);
}

#[test]
fn test_verify_cross_crate_version_success() {
    let env = Env::default();
    let result = crate::interface_spec_check::verify_cross_crate_version(&env, 1, "CrateA");
    assert!(result.is_ok());
}

#[test]
fn test_verify_cross_crate_version_failure() {
    let env = Env::default();
    let result = crate::interface_spec_check::verify_cross_crate_version(&env, 99, "CrateA");
    assert!(result.is_err());
}

#[test]
fn test_cross_crate_security_isolation() {
    let env = Env::default();
    let res_a = VerificationResult::new(&env);
    let mut res_b = VerificationResult::new(&env);
    res_b.add_error(&env, String::from_str(&env, "err"));
    assert!(res_a.passed);
    assert!(!res_b.passed);
}