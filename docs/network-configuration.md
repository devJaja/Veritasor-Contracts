# Cross-Network Configuration Contract

## Overview

The Network Configuration Contract (`veritasor-network-config`) provides a centralized, governable registry for network-specific parameters required to deploy and operate Veritasor contracts across multiple Stellar networks (e.g., Testnet, Mainnet, Futurenet).

This contract serves as the single source of truth for:
- Network-specific fee policies
- Allowed assets and their configurations
- Contract registry addresses
- Network parameters (block times, timeouts, limits)
- Governance and access control

## Architecture

### Design Principles

1. **Centralized Configuration**: Store all network-specific settings in one contract to avoid duplication and inconsistency
2. **Governance-Ready**: Support both admin and DAO-based governance for updates
3. **Non-Breaking Changes**: Add new networks without redeploying dependent contracts
4. **Security First**: Comprehensive access control with role-based permissions
5. **Read-Optimized**: Simple, efficient read APIs for other contracts to consume

### Network Identifier

Networks are identified by a `NetworkId` (u32). Suggested convention:

| NetworkId | Network   |
|-----------|-----------|
| 0         | Reserved  |
| 1         | Testnet   |
| 2         | Mainnet   |
| 3         | Futurenet |
| 4+        | Custom    |

## Data Structures

### NetworkConfig

Complete configuration for a Stellar network:

```rust
struct NetworkConfig {
    name: String,                          // Human-readable name
    network_passphrase: String,            // Stellar network passphrase
    is_active: bool,                       // Whether network is operational
    fee_policy: FeePolicy,                 // Fee configuration
    contracts: ContractRegistry,           // Related contract addresses (+ presence flags)
    block_time_seconds: u32,               // Average block time
    min_attestations_for_aggregate: u32,   // Min attestations to aggregate
    dispute_timeout_seconds: u64,          // Dispute resolution timeout
    max_period_length_seconds: u64,        // Max attestation period
    created_at: u64,                       // Creation timestamp
    updated_at: u64,                       // Last update timestamp
}
```

Approved assets are **not** embedded in `NetworkConfig`. Register them with `set_asset_config` and read the effective list via `get_allowed_assets` / `get_network_assets` (see [Storage layout](#storage-layout-for-assets-and-versions)).

### FeePolicy

Fee collection configuration:

```rust
struct FeePolicy {
    fee_token: Address,          // Token contract for payments
    fee_collector: Address,      // Address receiving fees
    base_fee: i128,             // Base fee in token units
    enabled: bool,              // Master fee toggle
    max_fee: i128,              // Fee cap (0 = unlimited)
    min_fee: i128,              // Fee floor
}
```

### AssetConfig

Asset-specific settings:

```rust
struct AssetConfig {
    asset_address: Address,    // Asset contract address
    asset_code: String,          // Asset code (e.g., "USDC")
    decimals: u32,              // Decimal places (max 18)
    is_active: bool,            // Whether asset is approved
    max_attestation_value: i128, // Max value per attestation
}
```

### ContractRegistry

Addresses of related Veritasor contracts:

```rust
struct ContractRegistry {
    attestation_contract: Address,
    revenue_stream_contract: Address,
    audit_log_contract: Address,
    aggregated_attestations_contract: Address,
    integration_registry_contract: Address,
    attestation_snapshot_contract: Address,
    has_attestation: bool,
    has_revenue_stream: bool,
    has_audit_log: bool,
    has_aggregated_attestations: bool,
    has_integration_registry: bool,
    has_attestation_snapshot: bool,
}
```

`get_contract_address` returns `Some` only when the corresponding `has_*` flag is true. Unused slots still hold an `Address` value but must be ignored when the flag is false.

## Access Control

### Role System

| Role        | Value | Capabilities                                    |
|-------------|-------|-------------------------------------------------|
| ADMIN       | 1     | Full control, role management, network removal  |
| GOVERNANCE  | 2     | Config updates, pause/unpause                  |
| OPERATOR    | 4     | Pause only                                     |

### Authorization Levels

- **Admin-only**: `remove_network`, role management, DAO changes
- **Governance+**: All config updates, unpause, network activation
- **Operator+**: Emergency pause

### Permission Flow

```
Emergency Pause    → OPERATOR, GOVERNANCE, ADMIN
Unpause           → GOVERNANCE, ADMIN only
Config Updates    → GOVERNANCE, ADMIN only
Network Removal   → ADMIN only
```

## Contract API

### Initialization

```rust
/// Initialize with admin and optional DAO
fn initialize(env: Env, admin: Address, governance_dao: Option<Address>)
```

**Example:**
```bash
stellar contract invoke --id <CONTRACT_ID> -- initialize \
  --admin <ADMIN_ADDRESS> \
  --governance_dao <DAO_ADDRESS>
```

### Role Management

```rust
/// Grant a role to an address (admin only)
fn grant_role(env: Env, caller: Address, account: Address, role: u32)

/// Revoke a role from an address (admin only)
fn revoke_role(env: Env, caller: Address, account: Address, role: u32)

/// Check if address has a specific role
fn has_role(env: Env, account: Address, role: u32) -> bool

/// Get all roles for an address (bitmap)
fn get_roles(env: Env, account: Address) -> u32

/// Get all addresses with any role
fn get_role_holders(env: Env) -> Vec<Address>
```

### Network Configuration

```rust
/// Set complete network configuration (governance+)
fn set_network_config(env: Env, caller: Address, network_id: NetworkId, config: NetworkConfig)

/// Update fee policy only (governance+)
fn update_fee_policy(env: Env, caller: Address, network_id: NetworkId, fee_policy: FeePolicy)

/// Add or update asset configuration (governance+)
fn set_asset_config(env: Env, caller: Address, network_id: NetworkId, asset_config: AssetConfig)

/// Remove asset from network (governance+)
fn remove_asset(env: Env, caller: Address, network_id: NetworkId, asset_address: Address)

/// Update contract registry (governance+)
fn update_contract_registry(env: Env, caller: Address, network_id: NetworkId, contracts: ContractRegistry)

/// Activate or deactivate network (governance+)
fn set_network_active(env: Env, caller: Address, network_id: NetworkId, active: bool)

/// Remove network (admin only, must be inactive)
fn remove_network(env: Env, caller: Address, network_id: NetworkId)

/// Set default network (governance+)
fn set_default_network(env: Env, caller: Address, network_id: NetworkId)

/// Set governance DAO address (admin only)
fn set_governance_dao(env: Env, caller: Address, dao: Address)
```

### Read APIs (Public)

```rust
/// Get complete network configuration
fn get_network_config(env: Env, network_id: NetworkId) -> Option<NetworkConfig>

/// Check if network is active
fn is_network_active(env: Env, network_id: NetworkId) -> bool

/// Get fee policy for network
fn get_fee_policy(env: Env, network_id: NetworkId) -> Option<FeePolicy>

/// Get all allowed assets
fn get_allowed_assets(env: Env, network_id: NetworkId) -> Vec<AssetConfig>

/// Get specific asset configuration
fn get_asset_config(env: Env, network_id: NetworkId, asset_address: Address) -> Option<AssetConfig>

/// Get contract registry
fn get_contract_registry(env: Env, network_id: NetworkId) -> Option<ContractRegistry>

/// Get specific contract address by name
fn get_contract_address(env: Env, network_id: NetworkId, contract_name: String) -> Option<Address>

/// Get all registered network IDs
fn get_registered_networks(env: Env) -> Vec<NetworkId>

/// Get default network ID
fn get_default_network(env: Env) -> NetworkId

/// Get network configuration version
fn get_network_version(env: Env, network_id: NetworkId) -> u32

/// Get global configuration version
fn get_global_version(env: Env) -> u32

/// Get network parameters tuple
fn get_network_parameters(env: Env, network_id: NetworkId) -> Option<(u32, u64, u64, u32)>

/// Validate asset for attestation
fn is_asset_valid_for_attestation(env: Env, network_id: NetworkId, asset_address: Address, amount: i128) -> bool

/// Get admin address
fn get_admin(env: Env) -> Address

/// Get governance DAO address
fn get_governance_dao(env: Env) -> Option<Address>

/// Check if contract is paused
fn is_paused(env: Env) -> bool
```

### Pause/Unpause

```rust
/// Pause contract (operator+)
fn pause(env: Env, caller: Address)

/// Unpause contract (governance+)
fn unpause(env: Env, caller: Address)
```

## Usage Examples

### 1. Deploy and Configure Testnet

```bash
# Initialize contract
stellar contract invoke --id <CONFIG_CONTRACT> -- initialize \
  --admin <ADMIN_ADDRESS> \
  --governance_dao <DAO_ADDRESS>

# Set testnet configuration
stellar contract invoke --id <CONFIG_CONTRACT> -- set_network_config \
  --caller <ADMIN_ADDRESS> \
  --network_id 1 \
  --config '{
    "name": "Testnet",
    "network_passphrase": "Test SDF Network ; September 2015",
    "is_active": true,
    "fee_policy": {
      "fee_token": "<USDC_TESTNET>",
      "fee_collector": "<FEE_COLLECTOR>",
      "base_fee": 1000000,
      "enabled": true,
      "max_fee": 10000000,
      "min_fee": 100000
    },
    "contracts": {
      "attestation_contract": "<ATTESTATION_CONTRACT>",
      "revenue_stream_contract": "<REVENUE_CONTRACT>",
      "audit_log_contract": "<AUDIT_CONTRACT>",
      "aggregated_attestations_contract": "<AGGREGATED_CONTRACT>",
      "integration_registry_contract": "<INTEGRATION_CONTRACT>",
      "attestation_snapshot_contract": "<SNAPSHOT_CONTRACT>",
      "has_attestation": true,
      "has_revenue_stream": true,
      "has_audit_log": true,
      "has_aggregated_attestations": true,
      "has_integration_registry": true,
      "has_attestation_snapshot": true
    },
    "block_time_seconds": 5,
    "min_attestations_for_aggregate": 10,
    "dispute_timeout_seconds": 86400,
    "max_period_length_seconds": 2592000,
    "created_at": 0,
    "updated_at": 0
  }'

# Set as default network
stellar contract invoke --id <CONFIG_CONTRACT> -- set_default_network \
  --caller <ADMIN_ADDRESS> \
  --network_id 1
```

### 2. Add Assets to Network

```bash
stellar contract invoke --id <CONFIG_CONTRACT> -- set_asset_config \
  --caller <ADMIN_OR_DAO> \
  --network_id 1 \
  --asset_config '{
    "asset_address": "<USDC_CONTRACT>",
    "asset_code": "USDC",
    "decimals": 7,
    "is_active": true,
    "max_attestation_value": 1000000000
  }'
```

### 3. Query Configuration from Another Contract

```rust
use veritasor_network_config::{NetworkConfigContractClient, FeePolicy};

// In your contract, query fee configuration
let config_client = NetworkConfigContractClient::new(env, &config_contract_id);
let fee_policy = config_client.get_fee_policy(&1u32); // Testnet

if let Some(policy) = fee_policy {
    if policy.enabled {
        // Collect fee using policy.base_fee, policy.fee_token, etc.
    }
}
```

### 4. Network Migration (Testnet → Mainnet)

```bash
# 1. Add mainnet configuration (initially inactive)
stellar contract invoke --id <CONFIG_CONTRACT> -- set_network_config \
  --caller <DAO_ADDRESS> \
  --network_id 2 \
  --config '{
    "name": "Mainnet",
    "network_passphrase": "Public Global Stellar Network ; September 2015",
    "is_active": false,  // Start inactive
    ...
  }'

# 2. Deploy mainnet contracts and update registry
stellar contract invoke --id <CONFIG_CONTRACT> -- update_contract_registry \
  --caller <DAO_ADDRESS> \
  --network_id 2 \
  --contracts '{...}'

# 3. Add mainnet assets
stellar contract invoke --id <CONFIG_CONTRACT> -- set_asset_config \
  --caller <DAO_ADDRESS> \
  --network_id 2 \
  --asset_config '{"asset_code": "USDC", ...}'

# 4. Activate mainnet
stellar contract invoke --id <CONFIG_CONTRACT> -- set_network_active \
  --caller <DAO_ADDRESS> \
  --network_id 2 \
  --active true

# 5. Switch default to mainnet
stellar contract invoke --id <CONFIG_CONTRACT> -- set_default_network \
  --caller <DAO_ADDRESS> \
  --network_id 2

# 6. Deactivate testnet when ready
stellar contract invoke --id <CONFIG_CONTRACT> -- set_network_active \
  --caller <DAO_ADDRESS> \
  --network_id 1 \
  --active false
```

### 5. Migration rollback (operational)

There is **no** dedicated on-chain rollback function. **Rollback** is performed by governance using the same write APIs:

- **Default network**: call `set_default_network` again to point at a still-active network (you cannot set the default to an inactive network).
- **Full or partial config**: call `set_network_config`, `update_fee_policy`, or `update_contract_registry` with a previously audited configuration snapshot.

**Version semantics (security / caching assumptions):**

- `get_global_version` increases on every successful governance mutation that the contract defines (including `set_default_network`, fee updates, assets, registry, activation, and full `set_network_config`).
- `get_network_version(network_id)` increases **only** when that network’s row is written via `set_network_config`, `update_fee_policy`, `update_contract_registry`, or `set_network_active` (not on asset-only updates). Integrators should not assume asset changes bump the per-network version counter.
- Counters **never decrease**; re-applying an older parameter snapshot restores *values* but not *history*.

While **paused**, all mutators that require an active contract (including rollback attempts) fail; reads continue to work.

### Storage layout for assets and versions

Per-network asset rows use dedicated storage keys (`NetworkAssetConfig`, `NetworkAssetAddresses`) so they do not collide with `NetworkVersion(network_id)`. Asset registration must go through `set_asset_config`; do not rely on embedding assets inside `NetworkConfig`.

## Integration Guide

### For Contract Developers

Integrate with the Network Config Contract to fetch network-specific parameters:

```rust
use soroban_sdk::{contract, contractimpl, Address, Env};
use veritasor_network_config::{NetworkConfigContractClient, FeePolicy};

#[contract]
pub struct MyContract;

#[contractimpl]
impl MyContract {
    pub fn do_something(env: Env, config_contract: Address, network_id: u32) {
        let config = NetworkConfigContractClient::new(&env, &config_contract);
        
        // Verify network is active
        assert!(
            config.is_network_active(&network_id),
            "Network not active"
        );
        
        // Get fee policy
        let fee_policy = config.get_fee_policy(&network_id)
            .expect("Fee policy not configured");
        
        // Use network parameters
        let params = config.get_network_parameters(&network_id)
            .expect("Network parameters not found");
        let (block_time, dispute_timeout, max_period, min_attestations) = params;
        
        // Your logic here...
    }
    
    pub fn get_attestation_contract(
        env: Env, 
        config_contract: Address, 
        network_id: u32
    ) -> Option<Address> {
        let config = NetworkConfigContractClient::new(&env, &config_contract);
        config.get_contract_address(&network_id, &"attestation".into())
    }
}
```

### Version tracking

Use `get_global_version()` for caching strategies:

```rust
// Check if cached config is stale
let current_version = config_client.get_global_version();
if current_version > cached_version {
    // Refresh cached configuration
}
```

## Events

| Event              | Topics              | Data                  | Description                      |
|--------------------|---------------------|-----------------------|----------------------------------|
| `init`             | -                   | admin                 | Contract initialized             |
| `net_set`          | network_id          | name                  | Network config created/updated   |
| `net_act`          | network_id          | active                | Network activation changed       |
| `fee_pol`          | network_id          | enabled               | Fee policy updated               |
| `asset`            | network_id          | asset_code            | Asset configuration changed      |
| `reg`              | network_id          | -                     | Contract registry updated        |
| `role_g`           | account             | (role, granter)       | Role granted                     |
| `role_r`           | account             | (role, revoker)       | Role revoked                     |
| `pause`            | -                   | caller                | Contract paused                  |
| `unpause`          | -                   | caller                | Contract unpaused                |
| `dao_set`          | -                   | dao                   | Governance DAO updated           |
| `def_net`          | -                   | network_id            | Default network changed          |

## Security Considerations

### Validation Rules

- `network_id` cannot be 0 (reserved)
- Network name and passphrase cannot be empty
- Base fee, min fee, max fee must be non-negative
- Max fee must be >= min fee (if max > 0)
- Block time: 1-3600 seconds
- Dispute timeout: minimum 1 hour
- Max period length: minimum 1 day
- Asset decimals: maximum 18
- Cannot remove default network
- Cannot remove active network
- Cannot revoke last admin role

### Emergency Procedures

1. **Pause Contract**: Any operator, governance, or admin can pause in emergency
2. **Unpause**: Requires governance or admin to restore operations
3. **Read Operations**: Continue to work while paused (query safety)
4. **Write Operations**: Blocked while paused (mutations require unpause)

## Test Coverage

The contract includes comprehensive tests covering:

- **Initialization**: Single initialization, double-init prevention, DAO setup
- **Access Control**: All role combinations, permission boundaries, lockout prevention
- **Network Management**: CRUD operations, validation, versioning
- **Fee Policies**: Updates, validation, edge cases
- **Asset Management**: Add, update, remove, validation
- **Registry Operations**: Updates, queries, partial configs
- **Pause/Unpause**: Role-based permissions, read vs write behavior
- **Governance**: DAO operations, role transitions
- **Network Migration**: Testnet to mainnet scenarios, partial migrations
- **Migration rollback**: Default pointer rollback, re-applying saved `NetworkConfig` / fee policy, monotonic versions, paused and inactive-network failure modes
- **Edge Cases**: Unknown networks, empty configs, boundary values

**Coverage**: 95%+ line coverage across all modules

### Running Tests

```bash
# From repository root (workspace)
cargo test -p veritasor-network-config

# Or from package directory
cd contracts/network-config
cargo test

# Run with output
cargo test -p veritasor-network-config -- --nocapture

# Migration / rollback focused examples
cargo test -p veritasor-network-config migration_rollback -- --nocapture
cargo test -p veritasor-network-config rollback -- --nocapture
```

## Deployment Checklist

- [ ] Deploy contract to target network
- [ ] Initialize with admin and DAO addresses
- [ ] Create network configurations for all supported networks
- [ ] Add allowed assets per network
- [ ] Populate contract registry with deployed contract addresses
- [ ] Set default network
- [ ] Grant appropriate roles to operators
- [ ] Verify all read APIs return expected values
- [ ] Test pause/unpause functionality
- [ ] Document network IDs and configuration for integrators

## Operational Guidance

### Adding a New Network

1. Choose an unused `network_id` (check `get_registered_networks()`)
2. Deploy all Veritasor contracts to the new network
3. Create network configuration with `set_network_config()`
4. Add allowed assets with `set_asset_config()`
5. Populate contract registry with deployed addresses
6. Test all operations before activating
7. Activate with `set_network_active(network_id, true)`
8. Optionally set as default with `set_default_network()`

### Updating Fee Policy

1. Prepare new `FeePolicy` with desired values
2. Call `update_fee_policy()` with governance/admin authorization
3. Verify with `get_fee_policy()`
4. Update integration documentation

### Emergency Response

1. **Identify issue**: Determine if contract needs immediate pause
2. **Pause**: Any authorized operator calls `pause()`
3. **Assess**: Review configuration for errors or attacks
4. **Fix**: Admin/governance makes necessary updates
5. **Test**: Verify fixes on read-only queries
6. **Resume**: Governance/admin calls `unpause()`

### Network Deprecation

1. Notify all integrators of planned deprecation
2. Ensure alternative network is configured and active
3. Deactivate network: `set_network_active(network_id, false)`
4. Update default network if needed: `set_default_network()`
5. After grace period, remove network: `remove_network(network_id)`

## Business Config: Immutable Anchor Fields

The Business Config contract supports **immutable anchor fields** — the ability to permanently lock individual configuration sections for a business so they can never be modified again.

### AnchorConfig

```rust
struct AnchorConfig {
    anomaly_policy_anchored: bool,
    integrations_anchored: bool,
    expiry_anchored: bool,
    custom_fees_anchored: bool,
    compliance_anchored: bool,
}
```

### Behavior

- **One-way lock**: Once a section is anchored (`true`), it cannot be un-anchored. Passing `false` for a previously anchored field is a no-op.
- **Per-business**: Anchor configurations are independent per business address.
- **Initial config allowed**: Anchoring fields before any config exists still permits the first `set_business_config` call. Immutability only applies to subsequent updates.
- **Granular enforcement**: Anchoring one section (e.g., compliance) does not affect updates to other sections (e.g., expiry).
- **Full and partial updates blocked**: Both `set_business_config` (full replace) and individual `update_*` methods respect anchor locks.

### API

```rust
/// Lock config sections for a business (admin only, irreversible)
fn set_anchor_config(env: Env, caller: Address, business: Address, anchor: AnchorConfig)

/// Query current anchor state (returns all-false if unset)
fn get_anchor_config(env: Env, business: Address) -> AnchorConfig
```

### Events

| Event      | Topics           | Data         | Description             |
|------------|------------------|--------------|-------------------------|
| `anc_set`  | business         | AnchorConfig | Anchor config updated   |

### Example

```bash
# Lock compliance config permanently for a regulated business
stellar contract invoke --id <BUSINESS_CONFIG_CONTRACT> -- set_anchor_config \
  --caller <ADMIN> \
  --business <BUSINESS> \
  --anchor '{
    "anomaly_policy_anchored": false,
    "integrations_anchored": false,
    "expiry_anchored": false,
    "custom_fees_anchored": false,
    "compliance_anchored": true
  }'
```

## License

Part of the Veritasor Contracts - see repository LICENSE file for details.
