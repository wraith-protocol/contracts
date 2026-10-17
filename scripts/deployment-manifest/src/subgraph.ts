import fs from 'node:fs';
import path from 'node:path';
import { isMap, isSeq, parseDocument, Scalar, type Document, type YAMLMap } from 'yaml';
import { EVM_PLACEHOLDER_ADDRESS, EXPECTED_CONTRACTS } from './catalogue.js';
import { readRecord, type Deployment } from './manifest.js';
import { DEPLOYMENTS_DIR, REPO_ROOT, recordPath, repoPath } from './paths.js';

/** Checked in. graph codegen, build and test read it directly. */
export const SUBGRAPH_TEMPLATE = 'evm/subgraph/subgraph.template.yaml';
/** Generated for one network and git-ignored. The only manifest `graph deploy` is given. */
export const SUBGRAPH_MANIFEST = 'evm/subgraph/subgraph.yaml';

// Filled in from the deployment record, so the template must not set them.
const RECORD_FIELDS = [['network'], ['source', 'address'], ['source', 'startBlock']];

const NETWORK_SLUG = /^[a-z0-9]+(-[a-z0-9]+)*$/;

export interface Rendered {
  manifest: string | null;
  errors: string[];
}

function dataSources(doc: Document): unknown[] | null {
  const seq = doc.get('dataSources');
  return isSeq(seq) ? seq.items : null;
}

/**
 * Why the template cannot be rendered: a data source that does not name an EVM
 * contract, or a value that belongs in the deployment record.
 */
export function templateErrors(source: string): string[] {
  const doc = parseDocument(source);
  if (doc.errors.length) return doc.errors.map((e) => e.message);
  const items = dataSources(doc);
  if (!items?.length) return ['has no dataSources'];
  const errors: string[] = [];
  items.forEach((ds, i) => {
    if (!isMap(ds)) {
      errors.push(`dataSources[${i}] is not a mapping`);
      return;
    }
    const where = `dataSource ${ds.get('name') ?? i}`;
    const abi = ds.getIn(['source', 'abi']);
    if (typeof abi !== 'string' || !EXPECTED_CONTRACTS.evm.includes(abi)) {
      errors.push(
        `${where}: source.abi ${abi} is not one of the EVM contracts (${EXPECTED_CONTRACTS.evm.join(', ')})`,
      );
    }
    for (const field of RECORD_FIELDS) {
      if (ds.hasIn(field)) {
        errors.push(
          `${where}: ${field.join('.')} is set; it comes from deployments/evm/<network>.json`,
        );
      }
    }
  });
  if (doc.has('templates')) {
    errors.push(
      'has templates (dynamic data sources); the generator only sets network on dataSources, so extend it before adding any',
    );
  }
  return errors;
}

function insertAt(map: YAMLMap, index: number, key: string, value: unknown, doc: Document) {
  map.items.splice(index, 0, doc.createPair(key, value));
}

const keyIndex = (map: YAMLMap, key: string) =>
  map.items.findIndex((pair) => (pair.key as Scalar | null)?.value === key);

/**
 * Fill the template's network, contract addresses and start blocks from one EVM
 * deployment record. Every data source needs its contract and a start block in
 * the record; anything missing is reported and nothing is rendered. There is no
 * placeholder fallback. `deployment` may be a record that failed validation (so
 * the contract named in each error can still be found), hence the defensive reads.
 */
export function renderSubgraph(
  template: string,
  deployment: Deployment,
  recordFile: string,
): Rendered {
  const errors = templateErrors(template).map((e) => `${SUBGRAPH_TEMPLATE}: ${e}`);
  if (errors.length) return { manifest: null, errors };

  const doc = parseDocument(template);
  const contracts = Array.isArray(deployment.contracts) ? deployment.contracts : [];
  for (const ds of dataSources(doc) as YAMLMap[]) {
    const name = ds.getIn(['source', 'abi']) as string;
    const contract = contracts.find((c) => c?.name === name);
    if (!contract) {
      errors.push(`${name}: not recorded in ${recordFile}`);
      continue;
    }
    const problems: string[] = [];
    const height = contract.deployment?.height;
    if (typeof height !== 'number' || !Number.isSafeInteger(height) || height < 0) {
      problems.push(`no start block (deployment.height) in ${recordFile}`);
    } else if (height === 0) {
      // 0 is the placeholder start block of the instant configs (validate-config.js).
      problems.push(`start block 0 in ${recordFile} is the placeholder, not a deployment block`);
    }
    if (String(contract.id).toLowerCase() === EVM_PLACEHOLDER_ADDRESS) {
      problems.push(`${recordFile} has the placeholder address ${contract.id}`);
    }
    if (problems.length) {
      errors.push(...problems.map((p) => `${name}: ${p}`));
      continue;
    }

    insertAt(ds, keyIndex(ds, 'name') + 1, 'network', deployment.network, doc);
    const source = ds.get('source') as YAMLMap;
    const address = new Scalar(contract.id);
    address.type = Scalar.QUOTE_SINGLE;
    insertAt(source, keyIndex(source, 'abi'), 'address', address, doc);
    insertAt(source, keyIndex(source, 'abi') + 1, 'startBlock', height, doc);
  }
  if (errors.length) return { manifest: null, errors };

  doc.commentBefore = [
    ` Generated from ${path.basename(SUBGRAPH_TEMPLATE)} and deployments/evm/${deployment.network}.json`,
    ` by scripts/deployment-manifest. Do not edit. Regenerate with`,
    `   npm run manifest -- --network ${deployment.network}`,
  ].join('\n');
  return { manifest: doc.toString({ lineWidth: 0 }), errors: [] };
}

/** Render the subgraph manifest for deployments/evm/<network>.json. */
export function generateSubgraph(network: string): Rendered {
  if (!NETWORK_SLUG.test(network)) {
    return { manifest: null, errors: [`"${network}" is not a network slug`] };
  }
  const file = recordPath('evm', network);
  const rel = path.relative(REPO_ROOT, file);
  if (!fs.existsSync(file)) {
    const dir = path.join(DEPLOYMENTS_DIR, 'evm');
    const known = fs.existsSync(dir)
      ? fs
          .readdirSync(dir)
          .filter((f) => f.endsWith('.json'))
          .map((f) => f.slice(0, -'.json'.length))
      : [];
    return {
      manifest: null,
      errors: [
        `no deployment record ${rel} (recorded EVM networks: ${known.join(', ') || 'none'})`,
      ],
    };
  }
  if (!fs.existsSync(repoPath(SUBGRAPH_TEMPLATE))) {
    return { manifest: null, errors: [`${SUBGRAPH_TEMPLATE} is missing`] };
  }

  const { deployment, errors } = readRecord('evm', file);
  const recordErrors = errors.map((e) => `${rel}: ${e}`);
  if (!deployment) return { manifest: null, errors: recordErrors };
  const rendered = renderSubgraph(
    fs.readFileSync(repoPath(SUBGRAPH_TEMPLATE), 'utf8'),
    deployment,
    rel,
  );
  if (recordErrors.length) {
    return { manifest: null, errors: [...recordErrors, ...rendered.errors] };
  }
  return rendered;
}

/** The network a generated manifest is for, or null unless its data sources name exactly one. */
export function generatedNetwork(source: string): string | null {
  const doc = parseDocument(source);
  if (doc.errors.length) return null;
  const networks = new Set(
    (dataSources(doc) ?? []).map((ds) => (isMap(ds) ? ds.get('network') : null)),
  );
  const [network] = networks;
  return networks.size === 1 && typeof network === 'string' ? network : null;
}
