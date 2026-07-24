# Stellar Contract ABIs

This directory contains ABI snapshots for the Stellar smart contracts in this repository.

## CI Flow

To prevent accidental, silent changes to contract ABIs (which would break downstream indexers and SDKs), our CI job diffs the current ABI against the snapshots in this directory. If a PR alters an ABI without updating the corresponding snapshot here, the CI will fail.

## How to Update

If you are intentionally changing a contract's function signatures, error variants, or event topics, you must update the ABI snapshots and include them in your PR.

Run the following command from the `stellar` directory:

```sh
pnpm abi:snapshot
```

This will rebuild the contracts and overwrite the JSON files in this directory with the updated ABIs. Commit the updated JSON files along with your code changes.
