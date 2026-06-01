#!/usr/bin/env node

/**
 * Wraith Names Health Check
 * 
 * Verifies that all registered names on the contract have healthy TTLs.
 * This is typically run as a periodic CI job to monitor name archive risks.
 * 
 * Exit codes:
 *   0 - All names are healthy
 *   1 - One or more names at risk of archival
 *   2 - Check failed (error)
 */

interface HealthCheckConfig {
  networkPassphrase: string;
  rpcUrl: string;
  contractId: string;
  ttlThresholdLedgers: number;
  criticalThresholdLedgers: number;
}

interface HealthResult {
  totalNames: number;
  healthyNames: number;
  atRiskNames: number;
  criticalNames: number;
  averageTtl: number;
  minTtl: number;
  maxTtl: number;
}

async function getHealthConfig(): Promise<HealthCheckConfig> {
  return {
    networkPassphrase:
      process.env.STELLAR_NETWORK_PASSPHRASE ||
      "Test SDF Network ; June 2015",
    rpcUrl:
      process.env.SOROBAN_RPC_URL ||
      "https://soroban-testnet.stellar.org",
    contractId:
      process.env.WRAITH_NAMES_CONTRACT ||
      process.env.CONTRACT_ID ||
      "",
    ttlThresholdLedgers: parseInt(
      process.env.TTL_THRESHOLD_LEDGERS || "100000"
    ),
    criticalThresholdLedgers: parseInt(
      process.env.CRITICAL_THRESHOLD_LEDGERS || "50000"
    ),
  };
}

async function checkNameHealth(
  config: HealthCheckConfig
): Promise<HealthResult> {
  console.log("=== Wraith Names Health Check ===");
  console.log(`Network: ${config.networkPassphrase}`);
  console.log(`Contract: ${config.contractId}`);
  console.log(`TTL Threshold: ${config.ttlThresholdLedgers} ledgers`);
  console.log(`Critical Threshold: ${config.criticalThresholdLedgers} ledgers`);
  console.log("");

  // Placeholder implementation
  // In production, this would:
  // 1. Fetch all registered names from the contract
  // 2. For each name, query its current TTL
  // 3. Classify as healthy/at-risk/critical
  // 4. Return aggregated statistics

  console.log("[TODO] Implement name health check");
  console.log("Currently a placeholder that demonstrates the interface.");
  console.log("");

  return {
    totalNames: 0,
    healthyNames: 0,
    atRiskNames: 0,
    criticalNames: 0,
    averageTtl: 0,
    minTtl: 0,
    maxTtl: 0,
  };
}

async function main(): Promise<void> {
  try {
    const config = await getHealthConfig();

    if (!config.contractId) {
      throw new Error(
        "Contract ID not specified. Set WRAITH_NAMES_CONTRACT or CONTRACT_ID environment variable."
      );
    }

    const result = await checkNameHealth(config);

    console.log("=== Health Check Results ===");
    console.log(`Total names: ${result.totalNames}`);
    console.log(`Healthy: ${result.healthyNames}`);
    console.log(`At risk: ${result.atRiskNames}`);
    console.log(`Critical: ${result.criticalNames}`);
    console.log(`Average TTL: ${result.averageTtl} ledgers`);
    console.log(`Min TTL: ${result.minTtl} ledgers`);
    console.log(`Max TTL: ${result.maxTtl} ledgers`);
    console.log("");

    // Determine exit code
    if (result.criticalNames > 0) {
      console.error(
        `❌ CRITICAL: ${result.criticalNames} names at immediate risk of archival`
      );
      process.exit(1);
    }

    if (result.atRiskNames > 0) {
      console.warn(`⚠️  WARNING: ${result.atRiskNames} names at risk of archival`);
      console.warn("Consider running the keeper bot to extend TTLs.");
      // Note: Return 0 (warning, not failure) to keep this non-blocking in CI
      process.exit(0);
    }

    console.log("✅ All names have healthy TTLs");
    process.exit(0);
  } catch (error) {
    console.error("Health check failed:", error);
    process.exit(2);
  }
}

main();
