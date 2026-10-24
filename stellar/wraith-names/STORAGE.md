# wraith-names Storage Architecture

## Storage Decisions

### Per-Name Data (Persistent)
- **Name → NameEntry**: Maps name hash to name, stealth meta-address, and owner
  - **Reason**: Scales with number of registered names; must survive contract upgrades
  - **TTL**: Extended on each access (resolve, update, release)
  
- **Subdomains**: `NameEntry.parent` holds the parent's name hash and
  `NameEntry.era` ties a subdomain to one registration of that parent.
  - **Subdomain era (Name Hash → u64)**: `DataKey::SubdomainEra` is created at
    0 when a top-level name gets its first subdomain and incremented every time
    that name is released. It is never decremented or deleted, so an era value
    is never reused for the same parent, even across re-registrations in the
    same ledger.
  - A subdomain stores the parent's era at registration and is live only while
    the two match. Releasing a parent therefore retires all of its subdomains
    without iterating over them: their entries are no longer renewed and
    expire, and registering the same subdomain name again replaces the dead
    entry (and its stale reverse lookup).
  - **Cost**: names that never have a subdomain get no era entry, and top-level
    entries store `era: None`. The counter is read only by subdomain
    operations and written only on release, so registering and resolving a
    top-level name do not touch it.
  - **TTL**: Extended with the parent whenever a subdomain is resolved or
    renewed, and on every write. If it is archived, subdomain operations and
    releasing the parent need it restored first, like any archived entry.

- **Reverse Lookup (Meta-Address Hash → Name Hash)**: Maps stealth meta-address hash to name hash
  - **Reason**: Lookup data that scales with registry size; durable
  - **TTL**: Extended alongside name entry

- **Replay Protection (Message Hash → Bool)**: Prevents signature replay in `*_on_behalf` calls
  - **Reason**: Security-critical; must be permanent to prevent old signatures from being replayed
  - **TTL**: Extended indefinitely (single write, rarely accessed)

### Contract Metadata (Instance) - REMOVED
- **Previous approach**: Some code extended instance storage
  - **Issue**: Instance storage is small, bounded, and doesn't scale well; wastes ledger resources
  - **New approach**: Only persistent entries are managed; instance TTL is skipped

## Migration Path
1. All per-name data was already persistent in the original codebase
2. Replay keys must use persistent storage (security-critical)
3. Remove all instance storage extends from `extend_ttls()` and TTL maintenance
4. The keeper service (#50) updates to extend only persistent keys

## TTL Constants
- `TTL_THRESHOLD`: 17,280 ledger intervals (~1 day)
- `TTL_EXTEND_TO`: 518,400 ledger intervals (~30 days)
