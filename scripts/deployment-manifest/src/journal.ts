import fs from 'node:fs';
import path from 'node:path';
import { sha256Hex } from './hash.js';
import { stellarInstance, wasmHashFromInstanceEntry } from './record/stellar.js';

export interface ContractJournalEntry {
  name: string;
  contractId: string;
  wasmHash: string;
  deployedAt: string;
  height?: number;
  verified: boolean;
}

export interface DeploymentJournal {
  version: 1;
  network: string;
  networkId: string;
  sourceCommit: string;
  deployer: string;
  startedAt: string;
  updatedAt: string;
  completedAt?: string;
  status: 'in_progress' | 'completed' | 'aborted';
  contracts: Record<string, ContractJournalEntry>;
}

export interface ResumeValidationResult {
  valid: boolean;
  mismatches: string[];
}

export interface PlanAction {
  contract: string;
  action: 'reuse' | 'deploy';
  reason: string;
  existingId?: string;
}

export class DeploymentJournalManager {
  private journalPath: string;

  constructor(journalPath: string) {
    this.journalPath = journalPath;
  }

  exists(): boolean {
    return fs.existsSync(this.journalPath);
  }

  read(): DeploymentJournal {
    if (!this.exists()) {
      throw new Error(`Deployment journal not found at ${this.journalPath}`);
    }
    const raw = fs.readFileSync(this.journalPath, 'utf8');
    return JSON.parse(raw) as DeploymentJournal;
  }

  write(journal: DeploymentJournal): void {
    journal.updatedAt = new Date().toISOString();
    const dir = path.dirname(this.journalPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    const tmp = `${this.journalPath}.tmp.${process.pid}.${Date.now()}`;
    const bak = `${this.journalPath}.bak`;
    fs.writeFileSync(tmp, JSON.stringify(journal, null, 2) + '\n', 'utf8');
    if (fs.existsSync(this.journalPath)) {
      try {
        fs.copyFileSync(this.journalPath, bak);
      } catch {}
    }
    fs.renameSync(tmp, this.journalPath);
  }

  initialize(
    network: string,
    networkId: string,
    sourceCommit: string,
    deployer: string,
  ): DeploymentJournal {
    const journal: DeploymentJournal = {
      version: 1,
      network,
      networkId,
      sourceCommit,
      deployer,
      startedAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      status: 'in_progress',
      contracts: {},
    };
    this.write(journal);
    return journal;
  }

  async validateResume(
    expectedNetwork: string,
    expectedNetworkId: string,
    expectedCommit: string,
    expectedDeployer: string,
    contractWasmPaths: Record<string, string>,
    rpcUrl?: string,
  ): Promise<ResumeValidationResult> {
    const mismatches: string[] = [];
    const journal = this.read();

    if (journal.network !== expectedNetwork) {
      mismatches.push(
        `Network mismatch: journal was created for "${journal.network}", resuming with "${expectedNetwork}"`,
      );
    }

    if (journal.networkId !== expectedNetworkId) {
      mismatches.push(
        `Network ID mismatch: journal has "${journal.networkId}", RPC reports "${expectedNetworkId}"`,
      );
    }

    if (journal.sourceCommit !== expectedCommit) {
      mismatches.push(
        `Source commit mismatch: journal has "${journal.sourceCommit}", current HEAD is "${expectedCommit}"`,
      );
    }

    if (journal.deployer.toLowerCase() !== expectedDeployer.toLowerCase()) {
      mismatches.push(
        `Deployer authority mismatch: journal has "${journal.deployer}", active identity is "${expectedDeployer}"`,
      );
    }

    for (const [name, entry] of Object.entries(journal.contracts)) {
      const wasmPath = contractWasmPaths[name];
      if (wasmPath && fs.existsSync(wasmPath)) {
        const currentHash = sha256Hex(fs.readFileSync(wasmPath));
        if (currentHash !== entry.wasmHash) {
          mismatches.push(
            `Artifact hash mismatch for contract "${name}": journal has "${entry.wasmHash}", current artifact is "${currentHash}"`,
          );
        }
      }

      if (rpcUrl && entry.contractId) {
        try {
          const instance = await stellarInstance(rpcUrl, entry.contractId);
          if (!instance) {
            mismatches.push(
              `On-chain contract missing for "${name}": ID ${entry.contractId} not found on network`,
            );
          } else {
            const onChainWasmHash = wasmHashFromInstanceEntry(instance.xdr);
            if (onChainWasmHash && entry.wasmHash && onChainWasmHash !== entry.wasmHash) {
              mismatches.push(
                `On-chain bytecode mismatch for "${name}": on-chain has ${onChainWasmHash}, journal recorded ${entry.wasmHash}`,
              );
            }
          }
        } catch (err) {
          mismatches.push(
            `Failed to query on-chain status for "${name}" (${entry.contractId}): ${(err as Error).message}`,
          );
        }
      }
    }

    return {
      valid: mismatches.length === 0,
      mismatches,
    };
  }

  recordContractStep(
    name: string,
    contractId: string,
    wasmPath: string,
    height?: number,
    verified: boolean = false,
  ): void {
    const journal = this.read();
    const wasmHash = sha256Hex(fs.readFileSync(wasmPath));
    journal.contracts[name] = {
      name,
      contractId,
      wasmHash,
      deployedAt: new Date().toISOString(),
      ...(height !== undefined ? { height } : {}),
      verified,
    };
    this.write(journal);
  }

  markContractVerified(name: string, height?: number): void {
    const journal = this.read();
    if (journal.contracts[name]) {
      journal.contracts[name].verified = true;
      if (height !== undefined) {
        journal.contracts[name].height = height;
      }
      this.write(journal);
    }
  }

  plan(requiredContracts: string[]): PlanAction[] {
    if (!this.exists()) {
      return requiredContracts.map((c) => ({
        contract: c,
        action: 'deploy',
        reason: 'No previous journal exists; fresh deployment required',
      }));
    }

    const journal = this.read();
    return requiredContracts.map((c) => {
      const entry = journal.contracts[c];
      if (entry && entry.contractId && entry.verified) {
        return {
          contract: c,
          action: 'reuse',
          reason: `Contract already deployed and verified at ${entry.contractId}`,
          existingId: entry.contractId,
        };
      }
      if (entry && entry.contractId && !entry.verified) {
        return {
          contract: c,
          action: 'reuse',
          reason: `Contract deployed at ${entry.contractId}; pending on-chain verification`,
          existingId: entry.contractId,
        };
      }
      return {
        contract: c,
        action: 'deploy',
        reason: 'Contract not yet deployed in this journal session',
      };
    });
  }

  markComplete(): void {
    const journal = this.read();
    journal.status = 'completed';
    journal.completedAt = new Date().toISOString();
    this.write(journal);
  }
}
