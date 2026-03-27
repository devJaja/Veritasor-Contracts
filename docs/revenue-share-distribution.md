# Revenue Share Distribution Contract

## Overview

The revenue-share contract distributes a caller-supplied revenue amount from a business wallet to a configured set of stakeholders using basis-point shares.

This contract currently does **not** read or validate attestations during `distribute_revenue`. The stored `attestation_contract` address is an integration hook and coordination reference for future use, not an active runtime dependency in the current implementation.

## Current Interface

Contract location: `contracts/revenue-share/src/lib.rs`

Primary tests: `contracts/revenue-share/src/test.rs`

### Admin and Replay-Protected Methods

All admin write methods use nonce-based replay protection on `NONCE_CHANNEL_ADMIN`.

- `initialize(admin, nonce, attestation_contract, token)`
- `configure_stakeholders(nonce, stakeholders)`
- `set_attestation_contract(nonce, attestation_contract)`
- `set_token(nonce, token)`
- `get_replay_nonce(actor, channel)`

Nonce rules:

- the first valid admin nonce is `0`
- each successful admin call increments the stored nonce by `1`
- replayed or skipped nonces are rejected
- failed calls revert without consuming the nonce

### Distribution Methods

- `distribute_revenue(business, period, revenue_amount)`
- `get_distribution(business, period)`
- `get_distribution_count(business)`
- `get_stakeholders()`
- `get_admin()`
- `get_attestation_contract()`
- `get_token()`
- `calculate_share(revenue, share_bps)`

## Stakeholder Configuration

Stakeholder shares are expressed in basis points.

- `1 bps = 0.01%`
- `10,000 bps = 100%`

Validation rules:

- stakeholder list must contain `1..=50` entries
- each stakeholder must have at least `1` bps
- stakeholder addresses must be unique
- total shares must equal exactly `10,000` bps

Stakeholder order is part of contract behavior because residual rounding is always assigned to index `0`.

## Distribution Algorithm

For each stakeholder:

`share_amount = revenue_amount * share_bps / 10_000`

The division uses integer truncation. After all truncated shares are computed:

- `residual = revenue_amount - sum(truncated_shares)`
- if `residual > 0`, the contract adds the full residual to the first stakeholder

This makes the allocation deterministic and ensures no value is lost to rounding.

## Distribution Invariants

The current test suite hardens the following invariants:

- total distributed amount always equals `revenue_amount`
- non-first stakeholders always receive their truncated base share exactly
- only the first stakeholder receives the rounding residual
- residual is deterministic for a given `(stakeholders, revenue_amount)` input
- residual is bounded by `stakeholder_count - 1`
- a `(business, period)` pair can only be distributed once
- failed distributions do not leave partial transfers or partial storage updates

## Residual Allocation Examples

### Example: Equal Three-Way Split

Shares:

- stakeholder 1: `3334`
- stakeholder 2: `3333`
- stakeholder 3: `3333`

For `revenue_amount = 10,001`:

- stakeholder 1 base share = `10,001 * 3334 / 10,000 = 3,334`
- stakeholder 2 base share = `10,001 * 3333 / 10,000 = 3,333`
- stakeholder 3 base share = `10,001 * 3333 / 10,000 = 3,333`
- base total = `10,000`
- residual = `1`
- final amounts = `[3,335, 3,333, 3,333]`

### Example: Tiny Revenue, Many Stakeholders

Shares:

- `50` stakeholders at `200` bps each

For `revenue_amount = 49`:

- every truncated base share is `0`
- residual = `49`
- final amounts = `[49, 0, 0, ..., 0]`

This is expected behavior. Very small revenues can collapse entirely to the first stakeholder because the contract prioritizes exact conservation and deterministic residual handling over proportional dust distribution.

## Failure Semantics

`distribute_revenue` panics if:

- stakeholders are not configured
- `revenue_amount` is negative
- the same `(business, period)` was already distributed
- token transfers fail

Important behavior:

- distribution state is recorded only after token transfers succeed
- if any transfer fails, the entire call reverts
- failed distributions do not increment `get_distribution_count`
- failed distributions do not persist `DistributionRecord`

## Security Assumptions

### Authorization

- admin configuration requires admin authorization plus the correct replay nonce
- revenue distribution requires `business.require_auth()`

### Atomicity

The contract relies on Soroban transaction atomicity for failure safety. Tests cover the case where a later stakeholder transfer fails after an earlier transfer would otherwise have succeeded, and assert that no partial payout remains.

### Attestation Assumption

The contract stores an `attestation_contract` address but does not currently validate that `revenue_amount` is backed by an attestation. Integrators must treat `distribute_revenue` as operating on a trusted caller-supplied amount unless they add external orchestration or a future contract revision introduces direct attestation reads.

### Deterministic Ordering

Residual allocation depends on stakeholder ordering. Reordering stakeholders changes who receives rounding dust even if basis-point totals are unchanged.

## Performance Characteristics

The contract is `O(n)` in the number of stakeholders for each distribution.

Per distribution call:

- `n` stakeholder reads
- `n` share calculations
- up to `n` token transfers
- one distribution record write
- one distribution counter update

Operational implications:

- gas cost grows linearly with stakeholder count
- the configured hard cap of `50` stakeholders keeps the worst-case loop bounded
- the maximum residual is less than the stakeholder count, so at most `49` tokens for the current cap

## Test Coverage and Assurance

The current suite covers:

- nonce replay protection for admin methods
- failed admin validation with nonce preservation
- exact split distributions
- zero-amount distributions
- residual allocation for skewed and equalized share sets
- tiny-revenue adversarial cases
- duplicate-period rejection with state preservation
- transfer-failure rollback behavior
- stakeholder reconfiguration affecting only future periods
- invariant matrix coverage across multiple share configurations and revenue values

Validation command:

```bash
cargo test -p veritasor-revenue-share
```

## Example Usage

```rust
let nonce = client.get_replay_nonce(&admin, &NONCE_CHANNEL_ADMIN);
client.configure_stakeholders(&nonce, &stakeholders);

client.distribute_revenue(
    &business,
    &String::from_str(&env, "2026-02"),
    &500_000,
);
```

## Future Integration

The stored `attestation_contract` field is intended to support a future flow where `distribute_revenue` or an adjacent orchestration layer derives `revenue_amount` from attested data instead of accepting it directly.
