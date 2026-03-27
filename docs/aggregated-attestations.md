# Aggregated Attestations

## Overview

The aggregated attestations contract aggregates attestation-derived metrics across sets of business addresses (portfolios) to power portfolio-level analytics for lenders and investors. It references core attestations via the snapshot contract and does not duplicate attestation data.

## Aggregation inputs and outputs

### Inputs

- **Portfolio** – A set of business addresses registered under a `portfolio_id` (e.g. a lender’s loan book).
- **Snapshot contract** – Address of the attestation-snapshot contract. Aggregation is computed by calling `get_snapshots_for_business` for each business in the portfolio.

### Outputs (summary metrics)

| Field                       | Type  | Description |
|----------------------------|-------|-------------|
| `total_trailing_revenue`    | i128  | Sum of `trailing_revenue` across all snapshot records for all businesses in the portfolio. |
| `total_anomaly_count`      | u32   | Sum of `anomaly_count` across all snapshot records. |
| `business_count`           | u32   | Number of businesses in the portfolio. |
| `businesses_with_snapshots`| u32   | Number of businesses that had at least one snapshot. |
| `average_trailing_revenue` | i128  | `total_trailing_revenue / businesses_with_snapshots`, or 0 if none. |

## Aggregation logic and limitations

- Aggregation is computed **on-demand** via cross-contract calls to the snapshot contract. No attestation or snapshot data is stored in the aggregated contract.
- For each business in the portfolio, the contract calls the snapshot contract’s `get_snapshots_for_business`. All returned snapshot records contribute to `total_trailing_revenue` and `total_anomaly_count` (sum over all periods for that business).
- **Duplicate businesses**: Portfolio registration rejects duplicate business addresses. As a defense-in-depth measure, aggregation also de-duplicates any legacy stored duplicates before computing metrics, so repeated entries do not inflate totals or counts.
- **Empty portfolios**: Unregistered or empty portfolio IDs return zeroed metrics.
- **Overlapping businesses**: The same business can appear in multiple portfolios; each portfolio’s metrics are independent.
- **Revoked attestations**: The aggregated contract does not re-check attestation revocation; the snapshot contract is the source of truth. If a snapshot was recorded for a later-revoked attestation, it still contributes to aggregates until the snapshot contract or indexing layer is updated.
- **Read-heavy usage**: The contract is designed for read-heavy access; writes are limited to admin registering/updating portfolios.

## API

- `initialize(admin)` – One-time setup.
- `register_portfolio(caller, portfolio_id, businesses)` – Set or replace the set of business addresses for a portfolio. Admin only. Duplicate business addresses are rejected.
- `get_aggregated_metrics(snapshot_contract, portfolio_id)` – Returns `AggregatedMetrics` by querying the snapshot contract for each business in the portfolio.
- `get_portfolio(portfolio_id)` – Returns the list of business addresses for a portfolio, if registered.

## Duplicate-Leaf Security Notes

This contract treats a portfolio as a **set** of business addresses, not a multiset.

- repeated business addresses must not amplify `total_trailing_revenue`
- repeated business addresses must not amplify `total_anomaly_count`
- repeated business addresses must not inflate `business_count`
- repeated business addresses must not inflate `businesses_with_snapshots`

The test suite includes both:

- a forward-looking registration check that rejects duplicates, and
- a legacy-state regression check proving that duplicate addresses already present in storage are de-duplicated at aggregation time

## Failure Semantics

- unauthorized `register_portfolio` calls panic
- duplicate-address portfolio registration panics
- failed duplicate registration leaves the previously stored portfolio unchanged

## Performance Notes

- registration now performs an `O(n^2)` duplicate scan over the submitted business vector
- aggregation performs an `O(n^2)` de-duplication pass before snapshot reads
- this extra work is bounded by portfolio size and is a deliberate tradeoff to prevent duplicate-leaf inflation bugs
