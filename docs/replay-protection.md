# Replay Protection (Nonce-Based)

This document describes the **nonce-based replay protection** used in Veritasor contracts to prevent reuse of signed messages or encoded calls.

## Overview

- **Per-actor, per-channel nonces**: Each logical actor (e.g. admin, business, multisig owner) has independent nonce streams per *channel*. Channels separate different classes of operations (e.g. admin vs business vs multisig).
- **Strict increment**: The first valid nonce for any `(actor, channel)` is `0`. Each successful call must supply the *current* stored nonce; on success the stored value is incremented by 1.
- **No reuse or skip**: Reusing a nonce or supplying a nonce other than the current one causes the call to panic. Skipping nonces is not allowed.
- **Overflow**: At `u64::MAX` the contract panics to avoid wrapping.

## Nonce Partitioning

Channels provide **namespace partitioning** that isolates nonce streams for different classes of operations. This ensures that:

- An admin operation cannot replay as a business operation (or vice versa).
- Multisig actions have their own independent ordering per owner.
- Governance actions cannot interfere with protocol operations.
- Each `(actor, channel)` pair is a completely independent nonce stream.

### Partitioning Invariants

1. **Cross-channel isolation**: Advancing the nonce on channel X has *no effect* on the nonce for the same actor on channel Y, for any X ≠ Y.
2. **Cross-actor isolation**: Advancing the nonce on channel X for actor A has *no effect* on the nonce for actor B on the same channel X, for any A ≠ B.
3. **Cartesian product independence**: The full set of nonce streams is the Cartesian product `actors × channels`. Each element of this product is an independent monotonic counter.

### Well-Known Channels

The `replay_protection` module defines a set of well-known channel constants that contracts SHOULD use for consistency:

| Constant              | Value | Usage |
|----------------------|-------|-------|
| `CHANNEL_ADMIN`      | 1     | Admin / role-authorized operations (init, configure, revoke, etc.) |
| `CHANNEL_BUSINESS`   | 2     | Business-initiated actions (attestation submissions, state mutations) |
| `CHANNEL_MULTISIG`   | 3     | Multisig owner actions (propose, approve, reject, execute) |
| `CHANNEL_GOVERNANCE`  | 4     | Governance-gated operations (proposals, voting, parameter updates) |
| `CHANNEL_PROTOCOL`   | 5     | Protocol-level automated operations (triggers, oracle updates) |

Contracts MAY define additional custom channels starting from `CHANNEL_CUSTOM_START` (256) to avoid collisions with potential future well-known constants.

### Channel Classification Helpers

The module provides classification functions:

- `is_well_known_channel(channel)` — returns `true` if the channel is in the range `1..=5`.
- `is_custom_channel(channel)` — returns `true` if the channel ≥ 256.
- Channel 0 and values 6–255 are neither well-known nor custom (reserved range).

## Storage Model

Replay state lives in each contract's instance storage under a shared key shape:

- **Key**: `ReplayKey::Nonce(Address, u32)` — actor address and channel id.
- **Value**: `u64` — next expected nonce (i.e. the value the caller must supply on the next call).

The implementation is in `contracts/common/src/replay_protection.rs` and is reused by contracts that depend on `veritasor-common`.

## API Reference

### Core Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_nonce` | `(env, actor, channel) -> u64` | Returns the current nonce for `(actor, channel)`. Returns `0` if unused. |
| `peek_next_nonce` | `(env, actor, channel) -> u64` | Alias for `get_nonce`; client-facing naming. |
| `verify_and_increment_nonce` | `(env, actor, channel, provided)` | Verifies `provided == current` and increments. Panics on mismatch or overflow. |

### Partition-Aware Bulk Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_nonces_for_channels` | `(env, actor, &[u32]) -> Vec<u64>` | Returns nonces for an actor across multiple channels in one call. Preserves input order. |
| `reset_nonce` | `(env, actor, channel)` | Resets nonce to 0. **No auth check** — caller must verify authorization. |
| `reset_nonces_for_channels` | `(env, actor, &[u32])` | Bulk reset across multiple channels. **No auth check**. |

## Attestation Contract: Channels and Entrypoints

The attestation contract defines three channels (see `NONCE_CHANNEL_*` in `contracts/attestation/src/lib.rs`):

| Channel constant              | Value | Actor / usage |
|------------------------------|-------|----------------|
| `NONCE_CHANNEL_ADMIN`        | 1     | Admin (or role-authorized caller) for admin operations |
| `NONCE_CHANNEL_BUSINESS`     | 2     | Business address for attestation submissions |
| `NONCE_CHANNEL_MULTISIG`     | 3     | Multisig owner (proposer, approver, rejecter, executor) |

**Admin channel (1)** is used for: `initialize`, `initialize_multisig`, `configure_fees`, `set_tier_discount`, `set_business_tier`, `set_volume_brackets`, `set_fee_enabled`, `configure_rate_limit`, `grant_role`, `revoke_role`, `pause`, `unpause`, `revoke_attestation`, `migrate_attestation`, and anomaly-related admin calls (`init`, `add_authorized_analytics`, `remove_authorized_analytics`). The *actor* is the address that authorizes the call (admin or role holder).

**Business channel (2)** is used for: `submit_attestation`, `submit_attestation_with_metadata`. The *actor* is the business address.

**Multisig channel (3)** is used for: `create_proposal`, `approve_proposal`, `reject_proposal`, `execute_proposal`. The *actor* is the multisig owner performing the action (proposer, approver, rejecter, or executor).

After `initialize(admin, 0)`, the next admin-channel nonce for `admin` is **1** (0 is consumed by `initialize`).

## Client Flow

1. **Query current nonce**  
   Call the contract's replay-nonce view (e.g. `get_replay_nonce(actor, channel)`) to get the value the caller must supply on the next state-mutating call for that `(actor, channel)`.

2. **Submit the call**  
   Invoke the entrypoint with that nonce (and any other args). The contract calls `replay_protection::verify_and_increment_nonce(env, &actor, channel, nonce)` at the start of the call (after auth/role checks).

3. **Retry on nonce mismatch**  
   If the call fails with a nonce mismatch (e.g. another transaction used the same nonce first), query `get_replay_nonce` again and retry with the new value.

4. **Bulk query (optional)**  
   For clients that interact across multiple channels, use `get_nonces_for_channels` to fetch all relevant nonces in a single call instead of issuing multiple queries.

Clients must not reuse or skip nonces; they should always use the value returned by `get_replay_nonce` for the next call.

## Security Notes

- **Authorization**: Replay protection is applied in addition to normal auth (e.g. `require_auth`, role checks). The actor passed to replay protection should match the address that authorizes the call.
- **Channels prevent cross-class replay**: Using separate channels (admin vs business vs multisig vs governance vs protocol) keeps nonce streams independent so one class of operation cannot replay or block another.
- **Strict ordering**: Enforcing "current nonce only" prevents replay and enforces a single linear history per (actor, channel), at the cost of requiring clients to track or query the current nonce and retry on conflict.
- **Channel ID collisions**: Contracts SHOULD use the well-known constants from `replay_protection` for standard operations and reserve custom channels (≥ 256) for contract-specific operations. Using the same channel ID for semantically different operations across contracts is safe because each contract has its own instance storage.
- **Reset safety**: The `reset_nonce` and `reset_nonces_for_channels` functions do **not** perform authorization checks. Calling contracts MUST verify the caller is authorized before invoking these functions. Resetting a nonce allows previously-used values to be valid again, which could enable replay attacks if used carelessly. Prefer key rotation over nonce reset in production.
- **Overflow protection**: At `u64::MAX`, the contract panics. Under normal usage (one nonce per transaction), `u64::MAX` is effectively unreachable.

## Integration with Access Control / Governance

- Admin and role-gated entrypoints use the **admin** channel (`CHANNEL_ADMIN`); the actor is the caller (admin or role holder).
- Multisig entrypoints use the **multisig** channel (`CHANNEL_MULTISIG`); the actor is the multisig owner performing the action. This integrates with existing multisig auth so that each owner has their own nonce stream for multisig actions.
- Governance entrypoints use the **governance** channel (`CHANNEL_GOVERNANCE`); the actor is the governance participant.

Other contracts (e.g. integration registry, audit log, revenue contracts) can adopt the same pattern by depending on `veritasor-common`, importing the well-known channel constants, and calling `verify_and_increment_nonce` at the start of each state-mutating entrypoint, with a view that exposes `replay_protection::get_nonce` (or `peek_next_nonce`) as `get_replay_nonce(actor, channel)`.

### Migration Guide for Existing Contracts

Contracts that currently define their own `NONCE_CHANNEL_ADMIN` (value 1), `NONCE_CHANNEL_BUSINESS` (value 2), etc. can migrate to the common constants:

```rust
// Before
const NONCE_CHANNEL_ADMIN: u32 = 1;

// After
use veritasor_common::replay_protection::CHANNEL_ADMIN;
```

The values are backwards-compatible (same numeric values), so existing stored nonces remain valid.
