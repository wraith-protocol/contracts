# Stellar Contract ABI Snapshots

This directory contains JSON snapshots of the Soroban contract ABIs. These snapshots serve as a CI gate to prevent unintentional or silent breaking changes to contract interfaces (such as function signatures, error variants, or event topics) that downstream indexers and SDKs rely on.

## Updating the ABI

If your PR intentionally modifies a contract's interface, you must update the snapshots. Otherwise, the CI job will fail.

To update the ABI snapshots, run the update script from the `stellar` directory:

```bash
./abi/update.sh
```

This will compile the contracts and overwrite the `.json` snapshots in this directory. Make sure to commit the updated snapshots in your PR.
