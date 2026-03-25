# Attestation Expiry Enforcement

## Overview

The attestation contract now enforces expiry at write time and at verification time.

- Expiry remains optional (`None` means no expiry).
- If expiry is present, it must be valid at submission time.
- Expired attestations remain stored and queryable for audit/history.
- `verify_attestation()` now treats expired attestations as invalid.

## Enforced Rules

When calling `submit_attestation(...)` with `expiry_timestamp = Some(expiry_ts)`:

1. `expiry_ts` MUST be strictly greater than `timestamp`
2. `expiry_ts` MUST be strictly greater than current ledger timestamp (`env.ledger().timestamp()`)

If either rule fails, submission panics with:

- `"expiry must be after attestation timestamp"` or
- `"expiry must be in the future"`

This prevents expired-on-arrival records and malformed time windows.

## Contract Behavior

### `submit_attestation`

Stores attestation only after passing expiry validation.

### `is_expired`

Returns:

- `true` when attestation exists, has expiry, and `ledger_time >= expiry`
- `false` otherwise (no attestation, no expiry, or still fresh)

### `verify_attestation`

Returns `true` only when all of the following hold:

1. Attestation exists
2. Attestation is not revoked
3. Attestation is not expired
4. Stored Merkle root equals supplied Merkle root

This is the core freshness enforcement entry point for contract consumers.

### `get_attestation`

Unchanged for auditability. Even expired attestations are still returned.

### `migrate_attestation`

Unchanged for expiry field semantics: migration preserves existing `expiry_timestamp`.

## Data Model

Attestation payload remains:

```rust
(BytesN<32>, u64, u32, i128, Option<BytesN<32>>, Option<u64>)
// (merkle_root, timestamp, version, fee_paid, proof_hash, expiry_timestamp)
```

No storage layout migration is required.

## Security Notes

- Enforcing future-only expiry prevents stale attestations from being created intentionally.
- Enforcing `expiry > timestamp` prevents inconsistent temporal claims.
- Verification-time enforcement ensures freshness checks are not accidentally skipped by consumers.
- Ledger time is validator-controlled; if an integration requires stricter time guarantees, pair on-chain checks with off-chain controls.

## Test Coverage

See `contracts/attestation/src/expiry_test.rs` for comprehensive coverage of:

- Submissions without expiry
- Valid future expiry submissions
- Rejection of past/edge/invalid expiry values
- Expiry boundary behavior (`ledger_time == expiry`)
- Verification failing after expiry
- Queryability of expired attestations
