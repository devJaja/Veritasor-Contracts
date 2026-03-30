# Security Invariants

This document lists the security invariants enforced by the Veritasor contracts and the dedicated invariant tests. The invariant tests live in `contracts/common/src/security_invariant_test.rs` and complement property and fuzz tests.

## Enforced invariants

### Attestation contract

#### Access Control & Authorization

- **Role-based access control (RBAC)**  
  Four distinct roles: ADMIN, ATTESTOR, BUSINESS, OPERATOR. Each role has specific permissions:
  - ADMIN: Full protocol control, can grant/revoke all roles
  - ATTESTOR: Can submit attestations on behalf of businesses
  - BUSINESS: Can submit own attestations, view own data
  - OPERATOR: Can perform routine operations (pause, unpause)

- **Strict authorization checks**  
  All sensitive operations require explicit authentication via `require_auth()` followed by role verification. Authentication always precedes authorization checks to prevent spoofing.

- **Role bitmap validation**  
  Role bitmaps must only use defined bits (0b1111 = 0xF). Attempts to set undefined bits are rejected with panic. This prevents invalid role states and undefined behavior.

- **Non-zero role requirement**  
  Granting or setting a zero-value role is rejected. Roles must be non-zero and within the valid range.

- **No unauthorized role grants**  
  Only an address with the ADMIN role can grant roles. An address without ADMIN that calls `grant_role` panics with "caller does not have ADMIN role".

- **No unauthorized writes to attestation data**  
  Attestation submission requires the business address to authorize (or the caller to have ATTESTOR role). Revocation and migration require ADMIN. These are enforced by `require_auth` and role checks.

- **Privilege escalation prevention**  
  Non-admin users cannot grant any roles, including ADMIN. Attempts by ATTESTOR, BUSINESS, or OPERATOR to grant roles result in panic.

- **Audit trail for role changes**  
  All role grants and revocations emit diagnostic events for off-chain monitoring and compliance auditing.

#### Replay Attack Prevention

- **Nonce validation required**  
  State-changing operations (grant_role, revoke_role, revoke_attestation) require strictly increasing nonces per account per channel. Nonce validation prevents replay attacks where an attacker re-submits a previously valid transaction.

- **Monotonically increasing nonces**  
  Each account maintains a last-used nonce per channel. New nonces must be strictly greater than the previous value. Zero and duplicate nonces are rejected.

- **Channel-based nonce separation**  
  Nonces are tracked per account per channel (e.g., ADMIN channel, BUSINESS channel), allowing independent nonce sequences for different operation types.

- **Replay attack mitigation**  
  Attempting to reuse a nonce results in panic with "invalid nonce: must be greater than previous nonce". This applies even across different recipient addresses.

#### Pause Mechanism

- **Pause gate**  
  When the contract is paused, attestation submission and other sensitive operations are blocked. Only ADMIN can pause/unpause; OPERATOR can pause but not unpause.

- **Pause state validation**  
  Operations that check pause state will panic with "contract is paused" if attempted while paused.

#### Single Initialization

- **One-time initialization**  
  The contract can be initialized only once. A second call to `initialize` panics with "already initialized".

### Integration registry

- **Single initialization**  
  The registry can be initialized only once. A second `initialize` panics with "already initialized".

- **No unauthorized provider registration**  
  Only addresses with governance role can register, enable, disable, or update providers. A non-governance address calling `register_provider` (or similar) panics (e.g. "caller does not have governance role").

### Attestation snapshot contract

- **Admin or writer for recording**  
  Only admin or an address with the writer role can call `record_snapshot`. Unauthorized callers panic with "caller must be admin or writer".

### Aggregated attestations contract

- **Admin-only portfolio registration**  
  Only the contract admin can register or update portfolios. Unauthorized callers panic with "caller is not admin".

## Attack Vectors Considered and Mitigated

### Unauthorized Access
- **Mitigation**: All sensitive operations require `require_auth()` + role check
- **Protection**: Authentication precedes authorization to prevent credential bypass
- **Enforcement**: Panic messages clearly indicate authorization failures

### Replay Attacks
- **Mitigation**: Strictly increasing nonces per account per channel
- **Protection**: First nonce must be >= 1, subsequent nonces must be > last_used
- **Enforcement**: Duplicate or decreasing nonces cause transaction to panic
- **Scope**: Nonce tracking prevents replay across all state-changing operations

### Privilege Escalation
- **Mitigation**: Only ADMIN can grant roles; non-admin users cannot escalate
- **Protection**: Role bitmap validation prevents setting arbitrary bits
- **Enforcement**: Invalid role values or unauthorized grant attempts panic
- **Audit Trail**: All role changes emit events for detection and compliance

### Input Validation Attacks
- **Mitigation**: Role bitmaps validated against ROLE_VALID_MASK (0b1111)
- **Protection**: Zero-value roles rejected; undefined bits rejected
- **Enforcement**: Invalid inputs cause immediate panic before state changes

### State Transition Attacks
- **Mitigation**: Business status transitions validated (Pending→Active, Active→Suspended, etc.)
- **Protection**: Invalid transitions (e.g., Pending→Suspended) are rejected
- **Enforcement**: Status machine enforces valid lifecycle paths

## Security Assumptions

1. **Admin Existence**: At least one address must always hold the ADMIN role
2. **Address Validity**: Soroban's `require_auth()` validates address ownership
3. **Nonce Channels**: Different operation types use separate nonce channels to prevent cross-context replay
4. **Event Monitoring**: Off-chain systems monitor role change events for anomalies
5. **Least Privilege**: Accounts start with no roles; privileges granted explicitly

The invariant tests are written so that:

- They **assert** the above behavior (e.g. second initialize panics, non-admin cannot grant role).
- Edge cases (e.g. empty portfolios, missing attestations) are covered in the respective contract test suites.
- New invariants can be added over time by appending tests in `security_invariant_test.rs` (and, if needed, per-contract modules) and documenting them here.

## How to add new invariants

1. **Define the invariant**  
   State clearly what must always hold (e.g. "no unbounded growth of X", "only Y can write Z").

2. **Encode it in a test**  
   In `contracts/common/src/security_invariant_test.rs` (or a per-contract invariant module), add a `#[test]` that:
   - Sets up the relevant contract(s).
   - Performs the action that should be forbidden or the condition that should never occur.
   - Asserts that the contract panics or returns an error, or that the state satisfies the invariant.

3. **Document**  
   Add a short bullet under the appropriate contract (or a new subsection) in this file.

4. **Run**  
   Invariant tests run with the rest of the test suite (`cargo test --all`) and in CI.
