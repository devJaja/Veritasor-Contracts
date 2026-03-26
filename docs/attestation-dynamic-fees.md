# Dynamic Fee Schedule for Attestations

## Overview

The Veritasor attestation contract supports a dynamic, on-chain fee schedule that adjusts attestation costs based on **business tier** and **cumulative volume**. Fees are denominated in a configurable Soroban token (e.g. USDC) and collected atomically during each `submit_attestation` call.

When fees are not configured or are disabled, attestations remain free — preserving full backward compatibility with the original contract behavior.

## Economic Rationale

### Why tiered + volume-based pricing?

| Goal | Mechanism |
|------|-----------|
| **Reward loyalty** | Volume discounts reduce per-unit cost as usage grows |
| **Reward commitment** | Tier discounts let enterprises negotiate better rates |
| **Predictable revenue** | Deterministic formula — no oracles, no off-chain state |
| **Fair compounding** | Multiplicative (not additive) discounts preserve protocol revenue at scale |

A 20% tier discount combined with a 10% volume discount yields a 28% total discount (not 30%). This multiplicative model ensures the protocol retains more revenue than naive additive discounting while still rewarding both axes of loyalty.

### Why basis points?

All discounts use **basis points** (1 bps = 0.01%, 10 000 bps = 100%) to avoid floating-point arithmetic entirely. The fee formula uses only integer multiplication and division, making it deterministic and auditable on-chain.

## Fee Calculation

```
effective_fee = base_fee
    × (10 000 − tier_discount_bps)
    × (10 000 − volume_discount_bps)
    ÷ 100 000 000
```

### Worked example

| Parameter | Value |
|-----------|-------|
| Base fee | 1 000 000 stroops |
| Business tier | 1 (Professional) |
| Tier 1 discount | 2 000 bps (20%) |
| Attestation count | 12 |
| Volume bracket ≥10 | 1 000 bps (10%) |

```
effective = 1 000 000 × (10 000 − 2 000) × (10 000 − 1 000) ÷ 100 000 000
         = 1 000 000 × 8 000 × 9 000 ÷ 100 000 000
         = 720 000 stroops
```

## Tier System

Businesses are assigned to tiers by the contract admin. Tiers are identified by `u32` level numbers:

| Tier | Name | Typical discount |
|------|------|-----------------|
| 0 | Standard | 0% (default for all businesses) |
| 1 | Professional | 10–20% |
| 2 | Enterprise | 30–50% |
| 3+ | Custom | Admin-defined |

The scheme is open-ended — any `u32` tier level can be configured with a discount.

Unassigned businesses default to tier 0.

## Volume Discount Brackets

Volume discounts are defined as parallel vectors of thresholds and discounts:

```
thresholds: [10, 50, 100]
discounts:  [500, 1000, 2000]   (in basis points)
```

This means:
- 0–9 attestations: no volume discount
- 10–49 attestations: 5% volume discount
- 50–99 attestations: 10% volume discount
- 100+ attestations: 20% volume discount

Brackets are evaluated highest-threshold-first. The cumulative attestation count for a business is tracked on-chain and incremented on each successful submission.

## Contract API

### Initialization

```
initialize(admin: Address)
```

One-time setup. Must be called before any admin method. The `admin` address must authorize the call.

### Admin Methods (require admin authorization)

| Method | Description |
|--------|-------------|
| `configure_fees(token, collector, base_fee, enabled)` | Set or update the fee token, collector address, base fee, and enabled flag |
| `set_tier_discount(tier, discount_bps)` | Set the discount for a tier level (0–10 000 bps) |
| `set_business_tier(business, tier)` | Assign a business to a tier |
| `set_volume_brackets(thresholds, discounts)` | Set volume discount brackets (parallel vectors, ascending thresholds) |
| `set_fee_enabled(enabled)` | Toggle fee collection without changing other config |

### Core Methods

| Method | Description |
|--------|-------------|
| `submit_attestation(business, period, merkle_root, timestamp, version)` | Submit attestation; collects fee if enabled; business must authorize |
| `get_attestation(business, period)` | Returns `(merkle_root, timestamp, version, fee_paid)` |
| `verify_attestation(business, period, merkle_root)` | Returns `true` if attestation exists and root matches |

### Read-Only Queries

| Method | Description |
|--------|-------------|
| `get_fee_config()` | Current fee configuration or None |
| `get_fee_quote(business)` | Fee the business would pay for its next attestation |
| `get_business_tier(business)` | Tier assigned to a business (0 if unset) |
| `get_business_count(business)` | Cumulative attestation count |
| `get_admin()` | Contract admin address |

## Storage Layout

All data is stored in Soroban instance storage under the `DataKey` enum:

| Key | Value | Description |
|-----|-------|-------------|
| `DataKey::Attestation(Address, String)` | `(BytesN<32>, u64, u32, i128)` | Attestation record with fee paid |
| `DataKey::Admin` | `Address` | Contract administrator |
| `DataKey::FeeConfig` | `FeeConfig` | Token, collector, base fee, enabled flag |
| `DataKey::TierDiscount(u32)` | `u32` | Discount bps for a tier level |
| `DataKey::BusinessTier(Address)` | `u32` | Tier assignment for a business |
| `DataKey::BusinessCount(Address)` | `u64` | Cumulative attestation count |
| `DataKey::VolumeThresholds` | `Vec<u64>` | Volume bracket thresholds |
| `DataKey::VolumeDiscounts` | `Vec<u32>` | Volume bracket discounts |

## Configuration Guide

### 1. Deploy and initialize

```bash
# Deploy the WASM
stellar contract deploy --network testnet --source <KEY> \
  target/wasm32-unknown-unknown/release/veritasor_attestation.wasm

# Initialize with admin address
stellar contract invoke --network testnet --source <ADMIN_KEY> \
  --id <CONTRACT_ID> -- initialize --admin <ADMIN_ADDRESS>
```

### 2. Configure fees

```bash
# Set base fee of 1 USDC (7 decimals = 10_000_000)
stellar contract invoke --network testnet --source <ADMIN_KEY> \
  --id <CONTRACT_ID> -- configure_fees \
  --token <USDC_CONTRACT_ID> \
  --collector <FEE_COLLECTOR_ADDRESS> \
  --base_fee 10000000 \
  --enabled true
```

### 3. Set up tiers

```bash
# Professional tier: 15% discount
stellar contract invoke --network testnet --source <ADMIN_KEY> \
  --id <CONTRACT_ID> -- set_tier_discount --tier 1 --discount_bps 1500

# Enterprise tier: 30% discount
stellar contract invoke --network testnet --source <ADMIN_KEY> \
  --id <CONTRACT_ID> -- set_tier_discount --tier 2 --discount_bps 3000

# Assign a business to Professional tier
stellar contract invoke --network testnet --source <ADMIN_KEY> \
  --id <CONTRACT_ID> -- set_business_tier \
  --business <BUSINESS_ADDRESS> --tier 1
```

### 4. Set up volume brackets

```bash
stellar contract invoke --network testnet --source <ADMIN_KEY> \
  --id <CONTRACT_ID> -- set_volume_brackets \
  --thresholds '[10, 50, 100]' \
  --discounts '[500, 1000, 2000]'
```

## Security Properties

- **Admin-gated**: All fee and tier configuration requires admin authorization
- **One-time initialization**: `initialize` can only be called once
- **Input validation**: Discounts capped at 10 000 bps, thresholds must be ascending, base fee must be non-negative
- **Atomic fee collection**: Token transfer happens within `submit_attestation` — if the transfer fails (insufficient balance, no approval), the entire transaction reverts
- **Business authorization**: `submit_attestation` requires the business address to authorize, preventing unauthorized submissions

## Test Coverage

**61 tests** covering:

### Unit Tests (`dynamic_fees_test.rs`)

- **Pure arithmetic** (7 tests): `compute_fee` with all discount combinations including edge cases (zero base, full discount)
- **Flat fee** (1 test): No discounts configured, full base fee charged
- **Tier discounts** (1 test): Standard/Professional/Enterprise fee quotes
- **Volume brackets** (1 test): Fee reduction as attestation count crosses thresholds
- **Combined discounts** (1 test): Tier + volume multiplicative stacking
- **Tier upgrade** (1 test): Mid-usage tier change reflects immediately
- **Fee toggling** (2 tests): Enable/disable fees, backward compatibility with no config
- **Initialization guard** (1 test): Double-initialize panics
- **Quote accuracy** (1 test): `get_fee_quote` matches actual token deduction
- **Validation** (5 tests): Mismatched brackets, unordered thresholds, discount overflow, negative base fee
- **Economic simulation** (1 test): 30 attestations across 3 businesses at different tiers with volume brackets — verifies exact protocol revenue
- **Core attestation** (4 tests): Submit, get, verify, duplicate prevention, count increment

### Property Tests (`property_test.rs`) — Fee Monotonicity

Property tests are implemented in two styles:

1. **Proptest macros** for pure functions (no `Env`): generates thousands of random inputs, checks properties, and auto-shrinks failures.
2. **Parametric contract-state tests** for invariants requiring Soroban `Env`: a representative input matrix iterates over cases with fresh `Env` per case.

#### §A–§J: Core Attestation Invariants (P1–P14, 14 tests)

| ID  | Invariant |
|-----|-----------|
| P1  | `0 ≤ compute_fee(b,t,v) ≤ b` for all valid inputs |
| P2  | `compute_fee(b,0,0) = b`; full discounts yield zero |
| P3  | `compute_fee` is monotonically non-increasing in each discount axis |
| P4  | `get_attestation` returns exactly what `submit_attestation` stored |
| P5  | `get_business_count` increases by exactly 1 per submission |
| P6  | `verify_attestation` iff `(exists ∧ ¬revoked ∧ stored_root = r)` |
| P7  | After revocation, `verify_attestation` returns false for **any** root |
| P8  | Duplicate `(business, period)` always panics |
| P9  | `migrate_attestation` panics iff `new_version ≤ old_version` |
| P10 | `set_tier_discount` panics iff `discount_bps > 10_000` |
| P11 | `set_volume_brackets` panics iff lengths mismatch or thresholds not ascending |
| P12 | Business A's state never affects Business B's |
| P13 | Submissions panic with "contract is paused" while paused |
| P14 | `get_fee_quote()` before submit equals actual token deduction |

#### §K: Pure Arithmetic Fee Monotonicity (P15–P20, 6 proptest properties)

| ID  | Invariant | Security implication |
|-----|-----------|----------------------|
| P15 | `compute_fee` is monotonically non-decreasing in `base_fee` | Revenue never decreases as base fee increases |
| P16 | Multiplicative decomposition: `fee(b,t,v) ≈ fee(b,t,0) × (10_000−v) / 10_000` (±1 truncation) | Validates independence of discount axes |
| P17 | Discount axis symmetry: `fee(b,t,v) = fee(b,v,t)` | Order of discount application is irrelevant |
| P18 | Additive over-discounts vs multiplicative: `fee(b,t+v,0) ≤ fee(b,t,v)` | Protocol captures more revenue under multiplicative model |
| P19 | A positive base increment never decreases the fee | Monotone growth — no fee inversion attacks |
| P20 | N calls to `compute_fee` sum to N × single call (referential transparency) | Pure function, no hidden state |

#### §L: End-to-End Contract Fee Monotonicity (P21–P23, 3 parametric tests)

| ID  | Invariant | Test scope |
|-----|-----------|------------|
| P21 | `get_fee_quote` is monotonically non-increasing as tier level increases | 6 tier levels (0–5), discount 0–100% |
| P22 | `get_fee_quote` is monotonically non-increasing as count crosses volume brackets | 5 checkpoints (0, 5, 10, 25, 50 submissions) |
| P23 | `fee_paid` stored in sequential attestation records is monotonically non-increasing | 20 sequential submissions with 3 volume brackets |

#### §M: Combined Tier + Volume Monotonicity (P24–P25, 1 parametric test)

| ID  | Invariant |
|-----|-----------|
| P24 | When both tier and volume discounts increase, fee never rises (12-row matrix) |
| P25 | Every contract quote exactly matches `compute_fee` for the same parameters |

#### §N: Boundary Arithmetic Precision (P26–P28, 3 tests)

| ID  | Invariant |
|-----|-----------|
| P26 | 15 exact spot-checks at critical BPS boundaries (0, 1, 9_999, 10_000 bps) |
| P27 | No i128 overflow at maximum safe base (1 trillion stroops) |
| P28 | A 1 bps discount strictly reduces the fee for bases ≥ 10_000 |

#### §O: Adversarial Fee Manipulation Resistance (P29–P32, 4 tests)

| ID  | Invariant | Adversarial scenario |
|-----|-----------|----------------------|
| P29 | Valid tier assignments (0–10_000 bps) never produce fee > base fee | Admin cannot inflate fees |
| P30 | Fee toggle on/off produces deterministic, idempotent quotes | No side-effects from toggling |
| P31 | Volume bracket with threshold=0 applies immediately (count = 0) | New-business edge case coverage |
| P32 | Rapid tier reassignment converges to last assigned tier | No state leakage from intermediate assignments |

#### §P: Regression and Determinism (P33–P35, 3 tests)

| ID  | Invariant |
|-----|-----------|
| P34 | Documented worked example: `compute_fee(1_000_000, 2_000, 1_000) = 720_000` — exact reproduction of the fee formula from this document |
| P35 | `get_fee_quote` is idempotent: 10 consecutive calls always return the same value without mutating state |
| P33 | Multi-business revenue regression: 3 businesses × 12 attestations with tier + volume brackets always produces exactly **29_452_500** stroops total protocol revenue |

## Fee Monotonicity: Assumptions and Expected Behavior

### Core Assumptions

1. **Tier discounts are non-decreasing in tier level** — the admin must configure tier discounts such that higher tier numbers correspond to higher (or equal) discount values. The contract does not enforce ordering between tiers; monotonicity is only guaranteed when the admin applies this discipline. Property tests P21 and P24 validate this assumption.

2. **Volume bracket discounts are non-decreasing in threshold order** — enforced by the contract's `set_volume_brackets` validation. Property tests P22 and P11 validate this.

3. **Integer truncation is deterministic** — the fee formula uses integer division (floor). This means the computed fee may be up to 1 stroop less than the exact fractional result. Property test P16 formally bounds this error at ±1, and P26 spot-checks exact values.

4. **`compute_fee` is a pure, stateless function** — it has no storage access and always returns the same output for the same inputs. Property tests P17, P20, and P35 validate referential transparency.

5. **Token transfers are atomic** — if a token transfer fails (insufficient balance or missing approval), the entire `submit_attestation` call reverts. The contract never updates state (count, attestation record) before a successful fee collection.

### Monotonicity Guarantees

| Property | Guarantee | Caveat |
|----------|-----------|--------|
| Fee ≤ base fee | Always | — |
| Fee ≥ 0 | Always | — |
| Fee non-increasing in tier discount | Always (pure formula) | Requires admin to assign strictly ascending discounts across tiers |
| Fee non-increasing as volume count grows | Always (pure formula) | Requires volume bracket discounts to be non-decreasing |
| Fee non-increasing over sequential submissions | Always (contract) | Assumes no tier downgrade and non-decreasing brackets |
| Multiplicative ≥ additive fee | Always (pure formula) | Only for combined discounts summing to ≤ 10_000 bps |

### Security Notes

- **Negative discount protection**: `set_tier_discount` and `set_volume_brackets` use `u32` parameters — negative discounts are structurally impossible.
- **Overflow protection**: `i128` with `overflow-checks = true` (see `Cargo.toml`). The maximum intermediate value (1_000_000_000_000 × 10_000 × 10_000 = 10^20) is well within `i128::MAX ≈ 1.7 × 10^38`.
- **Fee inflation resistance**: Property P29 confirms that no valid discount configuration (0–10_000 bps) can produce a fee above the base fee.
- **Idempotent queries**: Property P35 confirms `get_fee_quote` has no side effects — it cannot be used to manipulate state.

## Running Tests

```bash
# All tests (unit + property)
cd contracts/attestation
cargo test

# Property tests only
cargo test -- property_test

# Fee monotonicity tests only (§K–§P)
cargo test -- prop_fee_monotone prop_fee_multiplicative prop_fee_discount prop_additive prop_tier_upgrade prop_volume_bracket prop_sequential prop_combined prop_arithmetic prop_no_overflow prop_minimal prop_tier_assignment prop_fee_toggle prop_zero_threshold prop_tier_reassignment prop_regression prop_get_fee_quote

# With verbose output to see proptest case counts
cargo test -- --nocapture 2>&1 | grep -E "(PASSED|FAILED|running|proptest)"
```

