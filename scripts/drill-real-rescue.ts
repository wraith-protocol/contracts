import { spawnSync } from 'child_process';
import { writeFileSync } from 'fs';
import { randomBytes } from 'crypto';
import {
  Horizon, rpc, Keypair, TransactionBuilder, Operation, BASE_FEE, Networks, xdr, scValToNative,
} from '@stellar/stellar-sdk';
import {
  generateRecipient, scalarFromHex, senderDerive, recipientDerive, signWithScalar,
} from './stealth-derivation';
import { runRescue } from './rescue-stealth-funds';

const NETWORK = process.env.DRILL_NETWORK ?? 'futurenet';
const HORIZON = process.env.DRILL_HORIZON ?? 'https://horizon-futurenet.stellar.org';
const RPC = process.env.DRILL_RPC ?? 'https://rpc-futurenet.stellar.org';
const PASSPHRASE = Networks.FUTURENET;
const FUNDER = process.env.DRILL_FUNDER ?? 'drill-deployer';
const ANNOUNCER: string =
  process.env.DRILL_ANNOUNCER_ID ??
  (() => {
    throw new Error('DRILL_ANNOUNCER_ID env var is required');
  })();
const STUCK_AMOUNT = '20';

const horizon = new Horizon.Server(HORIZON);
const rpcServer = new rpc.Server(RPC);
const log = (m: string) => console.log(`[${new Date().toISOString()}] ${m}`);
const fail = (m: string): never => { throw new Error(`DRILL FAILED: ${m}`); };
const stroops = (s: string) => BigInt(s.replace('.', ''));

function funderSecret(): string {
  for (const sub of [['keys', 'secret', FUNDER], ['keys', 'show', FUNDER]]) {
    const r = spawnSync('stellar', sub, { encoding: 'utf8', shell: process.platform === 'win32' });
    const m = r.stdout?.match(/S[A-Z2-7]{55}/);
    if (r.status === 0 && m) return m[0];
  }
  return fail(`could not read secret for identity "${FUNDER}"`);
}

async function nativeBalance(addr: string): Promise<bigint | null> {
  try {
    const a = await horizon.loadAccount(addr);
    return stroops(a.balances.find((b) => b.asset_type === 'native')!.balance);
  } catch (e: any) {
    if (e?.response?.status === 404) return null;
    throw e;
  }
}

async function main() {
  const t0 = Date.now();
  const recipient = generateRecipient();
  const ephHex = randomBytes(32).toString('hex');
  const stuck = senderDerive(scalarFromHex(ephHex), recipient.metaAddress);
  const dest = Keypair.random();
  const funderSec = funderSecret();
  const funder = Keypair.fromSecret(funderSec);
  log(`stuck stealth address: ${stuck.address}`);

  // 1. Fund the stuck address (no announcement yet) and a destination account
  const fundTx = new TransactionBuilder(await horizon.loadAccount(funder.publicKey()), {
    fee: BASE_FEE, networkPassphrase: PASSPHRASE,
  })
    .addOperation(Operation.createAccount({ destination: stuck.address, startingBalance: STUCK_AMOUNT }))
    .addOperation(Operation.createAccount({ destination: dest.publicKey(), startingBalance: '5' }))
    .setTimeout(60).build();
  fundTx.sign(funder);
  const fundRes = await horizon.submitTransaction(fundTx);
  const stuckBefore = (await nativeBalance(stuck.address)) ?? fail('stuck account missing after funding');
  const destBefore = (await nativeBalance(dest.publicKey())) ?? fail('destination missing');
  log(`funded: stuck=${stuckBefore} stroops, dest=${destBefore} stroops (tx ${fundRes.hash})`);

  // 2. Rescue step 1: run the real rescue tool (derive, balance guard, real Soroban announce)
  const startLedger = (await rpcServer.getLatestLedger()).sequence;
  const rescue = await runRescue(
    {
      ephemeralKey: ephHex,
      recipientMetaAddress: recipient.metaAddress,
      amount: STUCK_AMOUNT,
      asset: 'XLM',
      announcerId: ANNOUNCER,
      rpc: RPC,
      horizon: HORIZON,
      networkPassphrase: PASSPHRASE,
      sourceSecret: funderSec,
      schemeId: 2,
    },
    { log: (m) => log(`  rescue tool: ${m}`) },
  );
  if (rescue.stealthAddress !== stuck.address) {
    fail(`rescue tool derived ${rescue.stealthAddress}, sender derived ${stuck.address}`);
  }
  const announceHash = rescue.txHash ?? fail('rescue tool returned no transaction hash');
  log(`announce tx (via rescue tool): ${announceHash}`);

  // 3. Recipient scans events and finds the payment
  let found: any = null;
  for (let i = 0; i < 20 && !found; i++) {
    const res = await rpcServer.getEvents({
      startLedger, filters: [{ type: 'contract', contractIds: [ANNOUNCER] }], limit: 50,
    });
    found = res.events.find((e: any) => e.txHash === announceHash);
    if (!found) await new Promise((r) => setTimeout(r, 2000));
  }
  if (!found) fail('announce event not found via RPC getEvents');
  const data = scValToNative(found.value) as [string, Uint8Array, Uint8Array];
  const ephFromEvent = Buffer.from(data[1]).toString('hex');
  if (data[0] !== stuck.address) fail(`event stealth address mismatch: ${data[0]}`);
  if (ephFromEvent !== stuck.ephPubHex) fail('event ephemeral key mismatch');
  const r = recipientDerive(recipient, ephFromEvent);
  if (r.address !== stuck.address) fail('recipient could not rederive the stuck address');
  log('recipient found the payment via the announcement and rederived the key');

  // 4. Rescue step 2: recipient sweeps the funds (account merge, signed with derived scalar)
  const mergeTx = new TransactionBuilder(await horizon.loadAccount(stuck.address), {
    fee: BASE_FEE, networkPassphrase: PASSPHRASE,
  })
    .addOperation(Operation.accountMerge({ destination: dest.publicKey() }))
    .setTimeout(60).build();
  const sig = signWithScalar(mergeTx.hash(), r.stealthScalar, r.prefix, r.stealthPubBytes);
  mergeTx.addDecoratedSignature(
    new xdr.DecoratedSignature({ hint: r.stealthPubBytes.subarray(28, 32), signature: sig }),
  );
  const mergeRes: any = await horizon.submitTransaction(mergeTx);
  const fee = BigInt(mergeRes.fee_charged ?? 100);
  log(`sweep tx: ${mergeRes.hash}`);

  // 5. Verify with balances
  const stuckAfter = await nativeBalance(stuck.address);
  const destAfter = (await nativeBalance(dest.publicKey())) ?? fail('destination missing after sweep');
  if (stuckAfter !== null) fail(`stuck account still exists with ${stuckAfter} stroops`);
  const gained = destAfter - destBefore;
  if (gained !== stuckBefore - fee) fail(`dest gained ${gained}, expected ${stuckBefore - fee}`);

  const seconds = ((Date.now() - t0) / 1000).toFixed(1);
  const evidence = {
    network: NETWORK, rescueTool: 'scripts/rescue-stealth-funds.ts', announcer: ANNOUNCER,
    stuckAddress: stuck.address, destination: dest.publicKey(), fundTx: fundRes.hash,
    announceTx: announceHash, sweepTx: mergeRes.hash, stuckBeforeStroops: stuckBefore.toString(),
    stuckAfter: 'account merged (removed)', destBeforeStroops: destBefore.toString(),
    destAfterStroops: destAfter.toString(), gainedStroops: gained.toString(),
    sweepFeeStroops: fee.toString(), elapsedSeconds: seconds,
  };
  const file = `drills/evidence-real-rescue-${Date.now()}.json`;
  writeFileSync(file, JSON.stringify(evidence, null, 2));
  log(`RESCUE VERIFIED in ${seconds}s. Evidence: ${file}`);
}

main().catch((e) => { console.error(e); process.exitCode = 1; });