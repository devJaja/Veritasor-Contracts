# TODO: Secure Extended Metadata Against Unbounded Storage Growth / DoS

## Steps

- [x] 1. Update `contracts/attestation/src/extended_metadata.rs`
  - Add ASCII-alphabetic validation to `validate_currency_code`
  - Add `remove_metadata` helper
  - Add module-level security documentation

- [x] 2. Update `contracts/attestation/src/lib.rs`
  - Fix corrupted `submit_attestation_with_metadata` function
  - Wire `revoke_attestation` to call `extended_metadata::remove_metadata`

- [x] 3. Update `contracts/attestation/src/extended_metadata_test.rs`
  - Add non-ASCII panic test
  - Add numeric panic test
  - Add symbol panic test
  - Add whitespace panic test
  - Add metadata removal on revocation test

- [x] 4. Update `docs/attestation-metadata.md`
  - Add Security Considerations section

- [x] 5. Compile check (cargo unavailable in this environment; documented)

