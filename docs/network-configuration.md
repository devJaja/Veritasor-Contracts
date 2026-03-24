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
    allowed_assets: Vec<AssetConfig>,      // Approved assets for attestations
    contracts: ContractRegistry,           // Related contract addresses
    block_time_seconds: u32,               // Average block time
    min_attestations_for_aggregate: u32,   // Min attestations to aggregate
    dispute_timeout_seconds: u64,          // Dispute resolution timeout
    max_period_length_seconds: u64,        // Max attestation period
    created_at: u64,                       // Creation timestamp
    updated_at: u64,                       // Last update timestamp
}
```

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
    attestation_contract: Option<Address>,
    revenue_stream_contract: Option<Address>,
    audit_log_contract: Option<Address>,
    aggregated_attestations_contract: Option<Address>,
    integration_registry_contract: Option<Address>,
    attestation_snapshot_contract: Option<Address>,
}
```

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
    "allowed_assets": [],
    "contracts": {
      "attestation_contract": "<ATTESTATION_CONTRACT>",
      "revenue_stream_contract": "<REVENUE_CONTRACT>",
      "audit_log_contract": "<AUDIT_CONTRACT>",
      "aggregated_attestations_contract": "<AGGREGATED_CONTRACT>",
      "integration_registry_contract": "<INTEGRATION_CONTRACT>",
      "attestation_snapshot_contract": "<SNAPSHOT_CONTRACT>"
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

### Version Tracking

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

### Business Config Immutable Fields (Business-Config Contract)

The Business Config contract enforces immutability guarantees for specific fields to ensure configuration integrity:

#### Immutable Fields

| Field | Description | Behavior |
|-------|-------------|----------|
| `business` | Business address identifier | Never changes after initial config creation; acts as primary storage key |
| `created_at` | Creation timestamp | Set once on initial config creation; preserved across all subsequent updates |

#### Mutable Fields

| Field | Description | Behavior |
|-------|-------------|----------|
| `version` | Configuration version | Increments by 1 on each update operation |
| `updated_at` | Last update timestamp | Changes to current ledger time on each update |
| All policy fields | Anomaly policy, integrations, expiry, fees, compliance | Can be updated via admin operations |

#### Immutability Guarantees

1. **Business Address Stability**: The business address serves as the primary key for business-specific configuration. Once set via `set_business_config`, the business field remains constant regardless of subsequent updates through any method (`update_anomaly_policy`, `update_integrations`, etc.)

2. **Created Timestamp Preservation**: The `created_at` timestamp is set exactly once during initial configuration creation and is never modified by update operations. This provides a reliable audit trail for when a business configuration was first established.

3. **Cross-Update Consistency**: All update operations (partial updates via specialized methods and full updates via `set_business_config`) preserve both immutable fields while modifying mutable fields appropriately.

#### Regression Test Coverage

The contract includes comprehensive immutable field regression tests covering:
- Business address immutability across all 6 update methods
- Created timestamp preservation across all update operations  
- Version increment correctness (mutable field behavior)
- Business isolation (configs properly keyed by business address)
- Full lifecycle tests (create, update, read cycles)
- Performance tests with many businesses
- Edge cases: rapid updates, boundary values, empty configs

**Test Location**: `contracts/business-config/src/test.rs`
**Test Categories**: `immutable_field_tests`, `adversarial_regression_tests`, `performance_regression_tests`

#### Known Design Considerations

- **Global Defaults**: When no custom business config exists, the contract returns global defaults. The business field in global defaults uses the admin/caller address as a placeholder and may change when global defaults are updated. This is a known design quirk - global defaults are not business-specific.
- **Mock Environment**: In test environments, ledger timestamps may be 0, which affects `created_at` and `updated_at` fields. Tests account for this by verifying immutability rather than specific timestamp values.

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
- **Edge Cases**: Unknown networks, empty configs, boundary values

**Coverage**: 95%+ line coverage across all modules

### Running Tests

```bash
# Run all network-config tests
cd contracts/network-config
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_network_migration_scenario -- --nocapture
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

## License

Part of the Veritasor Contracts - see repository LICENSE file for details.
