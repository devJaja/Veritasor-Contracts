# Lender Access List — Audit Trail & Security Documentation

> **Target file:** `contracts/lender-access-list/src/lib.rs`
> **Test file:** `contracts/lender-access-list/src/test.rs`
> **Schema version:** `EVENT_SCHEMA_VERSION = 1`

---

## 1. Overview

The Lender Access List contract maintains a governance-controlled allowlist of lender addresses permitted to rely on Veritasor attestations. This document covers the audit trail design, security invariants, dual-control model, and operational responsibilities.

---

## 2. Dual-Control Access Model

The contract implements a **three-tier privilege hierarchy**:

| Role | Storage Key | Granted By | Capabilities |
|------|-------------|------------|--------------|
| **Admin** | `DataKey::Admin` | Set at `initialize` | All operations; transfer admin; grant/revoke all roles |
| **Governance** | `DataKey::GovernanceRole(Address)` | Admin only | Manage lenders (`set_lender`, `remove_lender`) |
| **DelegatedAdmin** | `DataKey::DelegatedAdmin(Address)` | Admin only | Manage lenders (`set_lender`, `remove_lender`) only |

### Privilege Boundaries

- Governance holders **cannot** grant or revoke governance for other accounts.
- Governance holders **cannot** grant or revoke delegated admin roles.
- Governance holders **cannot** transfer admin.
- Delegated admins have identical lender-management scope to governance but **no** role-management capabilities.
- A lender address has **no** implicit privileges; it cannot self-enroll or self-upgrade.

### Dual-Control Rationale

The `require_lender_admin` check uses OR logic: `has_governance || has_delegated_admin`. This enables:

1. **Operational delegation** — day-to-day lender onboarding can be delegated to an operator (delegated admin) without exposing governance capabilities.
2. **Separation of duties** — governance role changes require admin authorization; lender changes require only lender-admin authorization.
3. **Least privilege** — delegated admins cannot escalate to governance or admin.

---

## 3. Audit Trail Design

### 3.1 On-Chain Record Fields

Every `Lender` record carries three audit fields updated on every write:

| Field | Type | Description |
|-------|------|-------------|
| `added_at` | `u32` | Ledger sequence when first enrolled. **Never changes after enrollment.** |
| `updated_at` | `u32` | Ledger sequence of the most recent `set_lender` or `remove_lender` call. |
| `updated_by` | `Address` | Address that authorized the most recent change. |

These fields allow on-chain audit queries without requiring event replay.

### 3.2 Event Catalog

All events are `#[contracttype]` structs (XDR-serializable) and follow the two-topic pattern: `(primary_symbol, entity_address)`.

| Event | Topic | Secondary Topic | Payload Type |
|-------|-------|-----------------|--------------|
| Lender enrolled/updated | `lnd_set` | lender address | `LenderEvent` |
| Lender removed | `lnd_rem` | lender address | `LenderEvent` |
| Governance granted | `gov_add` | account address | `GovernanceEvent` |
| Governance revoked | `gov_del` | account address | `GovernanceEvent` |
| Delegated admin granted | `del_add` | account address | `DelegatedAdminEvent` |
| Delegated admin revoked | `del_del` | account address | `DelegatedAdminEvent` |
| Admin transferred | `adm_xfer` | new admin address | `AdminTransferredEvent` |

> **Symbol length constraint:** All topic symbols are ≤ 9 bytes, satisfying the Soroban `symbol_short!` macro requirement.

### 3.3 LenderEvent — Rich Diff Payload

`LenderEvent` carries both the new state and the previous state, enabling off-chain indexers to reconstruct a full diff without additional storage reads:

```rust
pub struct LenderEvent {
    pub lender: Address,
    pub tier: u32,                        // new tier
    pub status: LenderStatus,             // new status
    pub changed_by: Address,              // actor
    pub previous_tier: Option<u32>,       // None on first enrollment
    pub previous_status: Option<LenderStatus>, // None on first enrollment
}
```

- `previous_tier` and `previous_status` are `None` on first enrollment (no prior record).
- On `remove_lender`, `previous_tier` captures the tier before removal, enabling detection of high-tier removals.

### 3.4 Secondary Topic for Efficient Indexing

Every event includes the affected entity address as a secondary topic:

```
topics = (event_type_symbol, entity_address)
```

This allows off-chain indexers to filter events by entity (e.g., "all events for lender X") without scanning all contract events.

### 3.5 Schema Versioning

`EVENT_SCHEMA_VERSION` (currently `1`) must be incremented whenever a breaking field change is made to any event struct. Off-chain indexers should check `get_event_schema_version()` and re-parse historical events on version change.

---

## 4. Security Invariants

The following invariants are enforced by the contract and validated by the test suite:

### 4.1 Authentication Before Authorization

```
caller.require_auth()  →  role check  →  state mutation
```

`require_auth()` is called as the **first** operation in every mutating function, before any storage read. This prevents:
- Spoofing: an attacker cannot pass the role check without Soroban-level authentication.
- TOCTOU: auth is checked before state is read.

### 4.2 Admin Uniqueness

There is exactly one admin address at any time, stored at `DataKey::Admin`. Admin transfer atomically replaces the stored address. The previous admin retains any governance role they held (separate key) until explicitly revoked.

### 4.3 Role Revocation is Immediate

Role revocations take effect in the same ledger. Any in-flight transaction from a revoked address will fail the role check on the next ledger close.

### 4.4 No Privilege Escalation

- Governance cannot grant governance to others (admin-only).
- Delegated admin cannot grant any role (admin-only).
- Enrolled lenders have no implicit privileges.

### 4.5 Lender Record Immutability on Removal

`remove_lender` does **not** delete the storage entry. The record is retained with `status = Removed` and `tier = 0`. This preserves the audit trail: `added_at`, `updated_at`, and `updated_by` remain queryable.

### 4.6 Global Lender List Deduplication

`append_lender_to_list` performs a linear scan before appending. A lender address appears at most once in `DataKey::LenderList` regardless of how many times `set_lender` is called.

### 4.7 Admin Self-Transfer Guard

`transfer_admin` panics if `new_admin == admin` to prevent accidental no-op transfers that would still emit a misleading event.

---

## 5. Storage Key Analysis

| Key | Storage Tier | Mutability | Notes |
|-----|-------------|------------|-------|
| `DataKey::Admin` | Instance | Mutable (transfer_admin) | Single address |
| `DataKey::GovernanceRole(Address)` | Instance | Mutable (grant/revoke) | Boolean flag |
| `DataKey::DelegatedAdmin(Address)` | Instance | Mutable (grant/revoke) | Boolean flag |
| `DataKey::Lender(Address)` | Instance | Mutable (set/remove) | Full `Lender` struct |
| `DataKey::LenderList` | Instance | Append-only | `Vec<Address>` |

All keys use instance storage. The lender list is bounded by governance operations (not user-driven), keeping storage growth controlled.

---

## 6. Reentrancy Analysis

Soroban's execution model is single-threaded per transaction. There are no cross-contract calls in this contract, eliminating reentrancy risk entirely. All state mutations are atomic within a single transaction.

---

## 7. Failure Modes and Error Messages

| Panic Message | Trigger Condition | Recovery |
|---------------|-------------------|----------|
| `"already initialized"` | `initialize` called twice | Deploy a new instance |
| `"not initialized"` | `get_admin` before `initialize` | Call `initialize` first |
| `"caller is not admin"` | Non-admin calls admin-only function | Use correct admin address |
| `"caller lacks lender admin privileges"` | Non-governance/non-delegated-admin calls `set_lender`/`remove_lender` | Grant appropriate role |
| `"lender not found"` | `remove_lender` on unenrolled address | Verify lender address |
| `"new_admin must differ from current admin"` | `transfer_admin` with same address | Use a different address |

---

## 8. Admin and Operator Responsibilities

### Admin Responsibilities

1. **Initialize once** with a trusted governance address.
2. **Grant governance** only to audited, trusted addresses.
3. **Grant delegated admin** only to operational addresses with limited scope.
4. **Revoke roles promptly** when an operator is offboarded or compromised.
5. **Transfer admin** only to a new address that has been verified to hold the corresponding private key.
6. **After admin transfer**, consider revoking the previous admin's governance role if they should no longer manage lenders.

### Governance / Delegated Admin Responsibilities

1. **Verify lender identity** before enrollment.
2. **Set appropriate tier** based on the lender's integration level.
3. **Remove lenders promptly** when they are offboarded or their access should be revoked.
4. **Do not share credentials** — each operator should have their own address.

---

## 9. Integration Guidance

Contracts integrating with the lender access list should:

1. Store the deployed `LenderAccessListContract` address in their own storage.
2. For tier-gated operations, call `is_allowed(caller, required_tier)` after `caller.require_auth()`.
3. Define per-operation minimum tier requirements and document them.
4. Never cache `is_allowed` results across ledgers — always query fresh.

Example integration pattern:

```rust
// In a lender-facing contract:
fn lender_operation(env: Env, caller: Address, access_list: Address) {
    caller.require_auth();

    let client = LenderAccessListContractClient::new(&env, &access_list);
    assert!(client.is_allowed(&caller, &1u32), "caller is not an allowed lender");

    // ... proceed with operation
}
```

---

## 10. Test Coverage Summary

The test suite in `contracts/lender-access-list/src/test.rs` covers:

| Category | Tests | Notes |
|----------|-------|-------|
| Initialization | 3 | Double-init guard, admin/governance setup |
| Admin transfer | 5 | Happy path, self-transfer guard, non-admin guard, event schema |
| Governance role | 7 | Grant, revoke, idempotent, non-holder revoke, auth guards, events |
| Delegated admin | 7 | Grant, revoke, idempotent, non-holder revoke, auth guards, events |
| Lender lifecycle | 9 | Enroll, update, tier=0, remove, re-enroll, dedup, multi-lender |
| Access checks | 6 | min_tier=0, unenrolled, exact match, removed, tier=0, u32::MAX |
| Audit trail | 3 | added_at preserved, updated_at changes, updated_by tracked |
| Event schema | 7 | lnd_set (enroll/update/tier=0), lnd_rem, previous fields |
| Dual control | 8 | Both roles can manage, revoked roles lose access, scope limits |
| Negative / auth | 8 | All unauthorized paths panic with correct messages |
| Self-revocation | 5 | Admin can revoke own governance, governance/delegated cannot self-revoke |
| Bulk operations | 5 | Bulk enroll, bulk remove, tier upgrades, multi-governance, multi-delegated |
| Race conditions | 3 | Last-writer-wins, grant-then-revoke, enroll-remove-reenroll cycle |
| Privilege escalation | 4 | Lender cannot self-enroll, self-remove, self-upgrade |
| Query correctness | 5 | None for unenrolled, empty list, active excludes tier=0/removed, all includes removed |
| Boundary values | 3 | u32::MAX tier, tier=1 minimum, empty metadata strings |
| Event ordering | 2 | All state changes emit events, read-only calls emit no events |

**Total: 90+ test cases** covering all public API paths, all error conditions, and all security-sensitive boundaries.

---

## 11. Known Limitations and Risk Acceptance

| Limitation | Risk Level | Justification |
|------------|------------|---------------|
| No lender suspension (only removal) | Low | Removal + re-enrollment covers the use case; suspension adds complexity without meaningful security benefit at this tier |
| No bulk enroll/remove API | Low | Governance operations are infrequent; individual calls are auditable and gas-bounded |
| Linear scan in `append_lender_to_list` | Low | Lender list is governance-controlled and expected to be small (< 1000 entries) |
| Instance storage for all keys | Low | Lender list size is bounded by governance operations; persistent storage not needed at current scale |
| No expiry / time-based revocation | Medium | Operators must manually revoke; acceptable for current governance model but should be revisited if automated expiry is needed |

---

## 12. Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-04-25 | Initial audit documentation. Added `transfer_admin`, secondary event topics, `previous_tier`/`previous_status` in `LenderEvent`, `AdminTransferredEvent`, `EVENT_SCHEMA_VERSION`, `get_event_schema_version`. Fixed `symbol_short!` symbols to ≤ 9 bytes. Added 90+ tests. |
