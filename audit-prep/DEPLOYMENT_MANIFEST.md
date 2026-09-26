# Deployment Manifest

## Overview

This document outlines the planned deployment configuration for Wraith Protocol Stellar contracts on mainnet. It serves as a reference for auditors to understand the deployment architecture, governance configuration, and operational procedures.

**Target Network:** Stellar Mainnet  
**Deployment Date:** TBD (Post-audit + remediation)  
**Coordinator:** [TBD]

---

## Deployment Timeline

### Phase 1: Audit & Remediation (Current)
- **Duration:** 3-4 weeks
- **Activities:**
  - Third-party security audit
  - Remediation of findings
  - Re-audit of critical changes

### Phase 2: Testnet Deployment
- **Duration:** 2 weeks
- **Activities:**
  - Deploy to Stellar Testnet
  - Integration testing with SDK
  - UI testing with demo app
  - Monitoring and log analysis

### Phase 3: Soak Period
- **Duration:** 2 weeks minimum
- **Activities:**
  - Community testing on testnet
  - Bug bounty program launch
  - Performance monitoring
  - Storage rent sustainability verification

### Phase 4: Mainnet Deployment
- **Duration:** 1 week
- **Activities:**
  - Deploy contracts to mainnet
  - Verify reproducible builds
  - Configure admin multisig
  - Initialize contracts
  - Publish contract addresses

### Phase 5: Post-Launch Monitoring
- **Duration:** Ongoing
- **Activities:**
  - 24/7 monitoring
  - Incident response readiness
  - Community support
  - Regular security reviews

---

## Contract Deployment Order

Contracts must be deployed in this specific order due to dependencies:

### 1. stealth-announcer (First)
**Purpose:** Event emission  
**Dependencies:** None  
**Init Required:** No  
**Configuration:** None

```bash
soroban contract deploy \
  --wasm build/stealth_announcer.wasm \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY]

# Returns: CONTRACT_ID_ANNOUNCER
```

---

### 2. stealth-registry (Second)
**Purpose:** Meta-address registry  
**Dependencies:** None  
**Init Required:** No  
**Configuration:** None

```bash
soroban contract deploy \
  --wasm build/stealth_registry.wasm \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY]

# Returns: CONTRACT_ID_REGISTRY
```

---

### 3. stealth-sender (Third)
**Purpose:** Atomic send + announce  
**Dependencies:** stealth-announcer  
**Init Required:** Yes  
**Configuration:** Announcer address, optional asset policy

```bash
# Deploy
soroban contract deploy \
  --wasm build/stealth_sender.wasm \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY]

# Returns: CONTRACT_ID_SENDER

# Initialize (CRITICAL: Do exactly once)
soroban contract invoke \
  --id CONTRACT_ID_SENDER \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY] \
  -- init \
  --announcer CONTRACT_ID_ANNOUNCER
```

---

### 4. wraith-names (Fourth)
**Purpose:** Name resolution  
**Dependencies:** None  
**Init Required:** No (future: admin config)  
**Configuration:** None (future: admin address)

```bash
soroban contract deploy \
  --wasm build/wraith_names.wasm \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY]

# Returns: CONTRACT_ID_NAMES
```

---

### 5. stealth-splitter (Optional)
**Purpose:** Payment splitting  
**Dependencies:** stealth-announcer  
**Init Required:** Yes  
**Configuration:** Announcer address

```bash
soroban contract deploy \
  --wasm build/stealth_splitter.wasm \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY]

# Returns: CONTRACT_ID_SPLITTER

soroban contract invoke \
  --id CONTRACT_ID_SPLITTER \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY] \
  -- init \
  --announcer CONTRACT_ID_ANNOUNCER
```

---

### 6. stealth-batch-sender (Optional)
**Purpose:** Batch operations  
**Dependencies:** stealth-announcer  
**Init Required:** Yes  
**Configuration:** Announcer address

```bash
soroban contract deploy \
  --wasm build/stealth_batch_sender.wasm \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY]

# Returns: CONTRACT_ID_BATCH_SENDER

soroban contract invoke \
  --id CONTRACT_ID_BATCH_SENDER \
  --network mainnet \
  --source [DEPLOYER_SECRET_KEY] \
  -- init \
  --announcer CONTRACT_ID_ANNOUNCER
```

---

## Governance Configuration

### Frozen Contracts (No Admin)

**stealth-announcer:**
- No admin role
- No upgrade mechanism
- Immutable forever

**stealth-registry:**
- No admin role
- No upgrade mechanism
- Immutable forever

### Upgradeable Contracts (Multisig + Timelock)

**stealth-sender:**
- **Admin:** 3-of-5 Multisig
- **Timelock:** 7 days (120,960 ledgers)
- **Pause:** Enabled (emergency only)

**wraith-names:**
- **Admin:** 3-of-5 Multisig
- **Timelock:** 7 days (120,960 ledgers)
- **Pause:** Not enabled
- **Renunciation Path:** Yes (future)

### Multisig Guardians

**Security Council (5 members):**

1. **Guardian 1:** @truthixify
   - Role: Technical Lead
   - Key: Hardware wallet (Ledger)
   
2. **Guardian 2:** @thebabalola
   - Role: Security Lead
   - Key: Hardware wallet (Trezor)
   
3. **Guardian 3:** @bbkenny
   - Role: Operations Lead
   - Key: Hardware wallet (Ledger)
   
4. **Guardian 4:** @richiey1
   - Role: Community Lead
   - Key: Hardware wallet (Trezor)
   
5. **Guardian 5:** @drips-wave
   - Role: Development Lead
   - Key: Hardware wallet (Ledger)

**Multisig Configuration:**
- Threshold: 3-of-5 signatures required
- Implementation: Stellar multisig account or custom contract
- Backup: Encrypted key shares stored offline

---

## Contract Addresses

### Mainnet (To Be Populated Post-Deployment)

```json
{
  "network": "mainnet",
  "passphrase": "Public Global Stellar Network ; September 2015",
  "contracts": {
    "stealth_announcer": "C[TBD]",
    "stealth_registry": "C[TBD]",
    "stealth_sender": "C[TBD]",
    "wraith_names": "C[TBD]",
    "stealth_splitter": "C[TBD]",
    "stealth_batch_sender": "C[TBD]"
  },
  "admin": {
    "multisig_address": "G[TBD]",
    "guardians": [
      "G[TBD_1]",
      "G[TBD_2]",
      "G[TBD_3]",
      "G[TBD_4]",
      "G[TBD_5]"
    ],
    "threshold": 3
  }
}
```

### Testnet (Current)

```json
{
  "network": "testnet",
  "passphrase": "Test SDF Network ; September 2015",
  "contracts": {
    "stealth_announcer": "C[TESTNET_ID]",
    "stealth_registry": "C[TESTNET_ID]",
    "stealth_sender": "C[TESTNET_ID]",
    "wraith_names": "C[TESTNET_ID]"
  }
}
```

---

## Storage Rent Configuration

### Initial TTL Extension

All contracts will have their instance storage extended to:

- **Initial TTL:** 30 days (518,400 ledgers)
- **Threshold:** 1 day (17,280 ledgers)
- **Extend To:** 30 days on access

### Keeper Bot Configuration

A keeper bot will monitor and extend storage rent:

- **Monitoring Interval:** Every 6 hours
- **Extension Trigger:** TTL < 7 days
- **Extension Amount:** 30 days
- **Funding:** Protocol treasury

---

## Monitoring & Observability

### Metrics Collection

**Tracked Metrics:**
- Contract invocations (per function)
- Transaction success/failure rates
- Gas usage patterns
- Storage rent status
- Event emission counts

**Tools:**
- Custom metrics indexer
- Grafana dashboards
- Prometheus alerts

### Alert Configuration

**Critical Alerts:**
- Contract invoke failure rate > 5%
- Storage TTL < 3 days
- Upgrade proposal created
- Pause mechanism triggered

**Warning Alerts:**
- Gas usage spike > 2x average
- Unusual batch sizes
- High error rates

### Logging

**Logged Events:**
- All contract deployments
- All contract invocations
- All upgrade proposals
- All admin actions

---

## Incident Response

### Response Team

**On-Call Rotation:**
- Primary: Technical Lead
- Secondary: Security Lead
- Escalation: Full Security Council

**Response SLA:**
- Critical (fund loss risk): 15 minutes
- High (contract unavailable): 1 hour
- Medium (degraded performance): 4 hours
- Low (monitoring issue): 24 hours

### Emergency Procedures

**Scenario 1: Critical Vulnerability Discovered**
1. Pause affected contracts (if upgradeable)
2. Notify community via status page
3. Develop and test fix
4. Deploy fix via upgrade process
5. Resume normal operations

**Scenario 2: Storage Rent Exhaustion**
1. Immediate TTL extension via keeper bot
2. Investigate root cause
3. Adjust keeper bot configuration
4. Ensure adequate treasury funding

**Scenario 3: Admin Key Compromise**
1. Emergency multisig rotation
2. Audit all recent admin actions
3. Notify community
4. Investigate compromise vector
5. Implement additional security measures

---

## Post-Deployment Verification

### Verification Checklist

Within 24 hours of deployment:

- [ ] Verify contract addresses published
- [ ] Verify reproducible build hashes match deployed WASMs
- [ ] Verify multisig configuration correct
- [ ] Verify init functions called correctly
- [ ] Verify storage rent funded
- [ ] Verify monitoring active
- [ ] Verify alerts configured
- [ ] Verify status page live
- [ ] Test end-to-end flows
- [ ] Community announcement published

### Build Verification

```bash
# Fetch deployed WASM
soroban contract fetch \
  --id [CONTRACT_ID] \
  --network mainnet \
  --out deployed.wasm

# Compare hash
sha256sum deployed.wasm
# Should match audit-prep/reference-checksums.txt
```

### Functional Testing

**Test Scenarios:**
1. Register keys in registry
2. Send native XLM via sender
3. Send issued asset via sender
4. Register name in wraith-names
5. Resolve name in wraith-names
6. Batch send via sender

**Expected Result:** All tests pass within 5 minutes

---

## Rollback Plan

If critical issues are discovered post-deployment:

### For Frozen Contracts (announcer, registry)
**No rollback possible.** Must deploy new version with different address.

**Mitigation:**
- Extensive pre-deployment testing
- Conservative upgrade schedule
- Community education on new addresses

### For Upgradeable Contracts (sender, names)
**Rollback via upgrade mechanism:**

1. Prepare rollback WASM (previous version)
2. Submit upgrade proposal
3. Wait 7-day timelock
4. Execute upgrade to previous version
5. Notify community of rollback

**Timeline:** 7 days minimum (timelock)

---

## Communication Plan

### Pre-Deployment

- [ ] Audit report published
- [ ] Remediation plan published
- [ ] Deployment date announced (1 week notice)
- [ ] Community Q&A session

### During Deployment

- [ ] Real-time status updates
- [ ] Contract addresses published immediately
- [ ] Verification instructions provided

### Post-Deployment

- [ ] Deployment success announcement
- [ ] Updated documentation
- [ ] SDK release with new addresses
- [ ] Demo app updated

---

## Documentation Updates

Post-deployment, update:

1. **README.md** - Add mainnet addresses
2. **SDK config** - Add mainnet network
3. **API docs** - Update contract references
4. **Integration guides** - Add mainnet examples
5. **Status page** - Show mainnet status

---

## Budget & Resources

### Deployment Costs

| Item | Cost (XLM) | Notes |
|---|---|---|
| Contract deployments (6x) | ~60 | ~10 XLM per contract |
| Init transactions (3x) | ~3 | ~1 XLM per init |
| Storage rent (30 days) | ~50 | Initial extension |
| Buffer | ~20 | Contingency |
| **Total** | **~133 XLM** | ~$13 at $0.10/XLM |

### Ongoing Costs

| Item | Monthly Cost | Notes |
|---|---|---|
| Storage rent extensions | ~15 XLM | Keeper bot |
| Monitoring infrastructure | ~$50 | Cloud hosting |
| On-call rotation | $0 | Volunteer guardians |
| **Total** | **~$55/month** | Sustainable |

---

## Appendix: Deployment Script

```bash
#!/bin/bash
# deploy-mainnet.sh
# IMPORTANT: Review and test on testnet first!

set -e

NETWORK="mainnet"
SOURCE_KEY="[DEPLOYER_SECRET_KEY]"

echo "🚀 Deploying Wraith Protocol to Stellar Mainnet"
echo "================================================"

# Deploy announcer
echo "📢 Deploying stealth-announcer..."
ANNOUNCER=$(soroban contract deploy \
  --wasm build/stealth_announcer.wasm \
  --network $NETWORK \
  --source $SOURCE_KEY)
echo "✅ Announcer deployed: $ANNOUNCER"

# Deploy registry
echo "📚 Deploying stealth-registry..."
REGISTRY=$(soroban contract deploy \
  --wasm build/stealth_registry.wasm \
  --network $NETWORK \
  --source $SOURCE_KEY)
echo "✅ Registry deployed: $REGISTRY"

# Deploy sender
echo "💸 Deploying stealth-sender..."
SENDER=$(soroban contract deploy \
  --wasm build/stealth_sender.wasm \
  --network $NETWORK \
  --source $SOURCE_KEY)
echo "✅ Sender deployed: $SENDER"

# Init sender
echo "🔧 Initializing stealth-sender..."
soroban contract invoke \
  --id $SENDER \
  --network $NETWORK \
  --source $SOURCE_KEY \
  -- init \
  --announcer $ANNOUNCER
echo "✅ Sender initialized"

# Deploy names
echo "📛 Deploying wraith-names..."
NAMES=$(soroban contract deploy \
  --wasm build/wraith_names.wasm \
  --network $NETWORK \
  --source $SOURCE_KEY)
echo "✅ Names deployed: $NAMES"

# Save addresses
cat > mainnet-addresses.json <<EOF
{
  "network": "mainnet",
  "deployed_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "contracts": {
    "stealth_announcer": "$ANNOUNCER",
    "stealth_registry": "$REGISTRY",
    "stealth_sender": "$SENDER",
    "wraith_names": "$NAMES"
  }
}
EOF

echo ""
echo "🎉 Deployment complete!"
echo "📄 Addresses saved to mainnet-addresses.json"
echo ""
echo "⚠️  NEXT STEPS:"
echo "1. Verify build hashes match deployed contracts"
echo "2. Configure multisig admin"
echo "3. Extend storage TTL"
echo "4. Update documentation"
echo "5. Announce to community"
```

---

**Last Updated:** 2026-06-26  
**Document Version:** 1.0.0  
**Deployment Status:** Pre-Audit (Not Yet Deployed)
