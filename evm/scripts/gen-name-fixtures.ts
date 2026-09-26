import { ethers } from 'ethers';

const ETH_PREFIX = ethers.toUtf8Bytes('\x19Ethereum Signed Message:\n32');

function ethSignedHash(digestBytes32: string): Uint8Array {
  return ethers.getBytes(
    ethers.keccak256(ethers.concat([ETH_PREFIX, ethers.getBytes(digestBytes32)])),
  );
}

function sign(wallet: ethers.Wallet, digestBytes32: string): string {
  const sig = wallet.signingKey.sign(ethSignedHash(digestBytes32));
  return sig.r.slice(2) + sig.s.slice(2) + ethers.toBeHex(ethers.toNumber(sig.v), 1).slice(2);
}

function compress(wallet: ethers.Wallet): string {
  return wallet.signingKey.compressedPublicKey.slice(2);
}

const names = ['wraith', 'alice', 'bob', 'carlo', 'daria', 'eliot', 'felix', 'greta'];

// One unique spending+viewing keypair per name, so reverse lookup is exact.
const keys: Record<string, ethers.Wallet> = {};
for (const n of names) {
  const sk = ethers.keccak256(ethers.toUtf8Bytes('spending-' + n));
  const vk = ethers.keccak256(ethers.toUtf8Bytes('viewing-' + n));
  keys[n + 's'] = new ethers.Wallet(sk);
  keys[n + 'v'] = new ethers.Wallet(vk);
}

// Alternate viewing+spending for the "wraith" update fixture.
const altSpend = new ethers.Wallet(ethers.keccak256(ethers.toUtf8Bytes('spending-wraith-alt')));
const altView = new ethers.Wallet(ethers.keccak256(ethers.toUtf8Bytes('viewing-wraith-alt')));

function meta(s: ethers.Wallet, v: ethers.Wallet): string {
  return '0x' + compress(s) + compress(v);
}

for (const n of names) {
  const m = meta(keys[n + 's'], keys[n + 'v']);
  const regDigest = ethers.keccak256(ethers.solidityPacked(['string', 'bytes'], [n, m]));
  const relDigest = ethers.keccak256(ethers.solidityPacked(['string'], [n]));
  console.log(`bytes constant META_${n.toUpperCase()} = hex"${m}";`);
  console.log(
    `bytes constant SIG_REGISTER_${n.toUpperCase()} = hex"${sign(keys[n + 's'], regDigest)}";`,
  );
  console.log(
    `bytes constant SIG_RELEASE_${n.toUpperCase()} = hex"${sign(keys[n + 's'], relDigest)}";`,
  );
}
const altMeta = meta(altSpend, altView);
const updDigest = ethers.keccak256(ethers.solidityPacked(['string', 'bytes'], ['wraith', altMeta]));
console.log(`bytes constant META_WRAITH_ALT = hex"${altMeta}";`);
console.log(`bytes constant SIG_UPDATE_WRAITH = hex"${sign(keys['wraiths'], updDigest)}";`);
