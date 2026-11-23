import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadBindingInventory } from './bindings-inventory.js';

const stellar = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const { contracts } = loadBindingInventory(stellar);
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'wraith-binding-packages-'));
// pnpm's npm_execpath points to pnpm; invoke npm explicitly on both platforms.
function npm(args: string[], cwd: string, capture = false): string {
  const executable = process.platform === 'win32' ? 'cmd.exe' : 'npm';
  const command = process.platform === 'win32' ? ['/d', '/s', '/c', 'npm', ...args] : args;
  return (
    execFileSync(executable, command, {
      cwd,
      encoding: 'utf8',
      stdio: capture ? 'pipe' : 'inherit',
      env: {
        ...process.env,
        npm_config_ignore_scripts: 'true',
        npm_config_audit: 'false',
        npm_config_fund: 'false',
      },
    }) ?? ''
  );
}
try {
  const build = path.join(temporary, 'build');
  const consumer = path.join(temporary, 'consumer');
  const tarballs = path.join(temporary, 'tarballs');
  fs.mkdirSync(build);
  fs.mkdirSync(consumer);
  fs.mkdirSync(tarballs);
  const fixtureRoot = path.join(stellar, 'scripts/fixtures/bindings');
  const fixturePackage = JSON.parse(
    fs.readFileSync(path.join(fixtureRoot, 'package.json'), 'utf8'),
  );
  const fixtureLock = JSON.parse(
    fs.readFileSync(path.join(fixtureRoot, 'package-lock.json'), 'utf8'),
  );
  const dependencies: Record<string, string> = {};
  for (const contract of contracts) {
    const source = path.join(stellar, 'bindings/typescript', contract.name);
    const destination = path.join(build, contract.name);
    for (const name of ['src', 'package.json', 'tsconfig.json', 'README.md'])
      fs.cpSync(path.join(source, name), path.join(destination, name), { recursive: true });
    const pkg = JSON.parse(fs.readFileSync(path.join(destination, 'package.json'), 'utf8'));
    for (const [name, version] of Object.entries({ ...pkg.dependencies, ...pkg.devDependencies })) {
      if (dependencies[name] && dependencies[name] !== version)
        throw new Error(`Conflicting package dependency ${name}`);
      dependencies[name] = version as string;
    }
  }
  fs.copyFileSync(path.join(fixtureRoot, 'package.json'), path.join(build, 'package.json'));
  fs.copyFileSync(
    path.join(fixtureRoot, 'package-lock.json'),
    path.join(build, 'package-lock.json'),
  );
  npm(['ci', '--ignore-scripts'], build);
  const tsc = path.join(build, 'node_modules/typescript/bin/tsc');
  const installed: Record<string, string> = {};
  const packedEntries: Record<string, unknown> = {};
  for (const contract of contracts) {
    const directory = path.join(build, contract.name);
    for (const flags of [[], ['--noEmit']])
      execFileSync(process.execPath, [tsc, '-p', directory, ...flags], {
        cwd: build,
        stdio: 'inherit',
      });
    const result = JSON.parse(
      npm(
        ['pack', directory, '--pack-destination', tarballs, '--ignore-scripts', '--json'],
        build,
        true,
      ),
    );
    if (
      !result[0].files.some((file: { path: string }) => file.path === 'dist/index.js') ||
      !result[0].files.some((file: { path: string }) => file.path === 'dist/index.d.ts')
    )
      throw new Error(`Incomplete package: ${contract.name}`);
    installed[contract.name] =
      'file:' + path.join(tarballs, result[0].filename).replaceAll('\\', '/');
    const metadata = JSON.parse(fs.readFileSync(path.join(directory, 'package.json'), 'utf8'));
    packedEntries[`node_modules/${contract.name}`] = {
      version: metadata.version,
      resolved: installed[contract.name],
      integrity: result[0].integrity,
      dependencies: metadata.dependencies,
    };
  }
  const consumerPackage = {
    ...fixturePackage,
    dependencies: { ...fixturePackage.dependencies, ...installed },
  };
  fixtureLock.packages[''].dependencies = consumerPackage.dependencies;
  Object.assign(fixtureLock.packages, packedEntries);
  fs.writeFileSync(path.join(consumer, 'package.json'), JSON.stringify(consumerPackage));
  fs.writeFileSync(path.join(consumer, 'package-lock.json'), JSON.stringify(fixtureLock));
  npm(['ci', '--ignore-scripts'], consumer);
  const imports = contracts
    .map(
      (contract, index) =>
        `import { Client as Client${index} } from '${contract.name}';\nif (typeof Client${index} !== 'function') throw new Error('${contract.name} has no Client export');`,
    )
    .join('\n');
  fs.writeFileSync(path.join(consumer, 'smoke.ts'), imports);
  fs.writeFileSync(path.join(consumer, 'smoke.mjs'), imports);
  execFileSync(
    process.execPath,
    [
      path.join(consumer, 'node_modules/typescript/bin/tsc'),
      '--noEmit',
      '--strict',
      '--skipLibCheck',
      '--target',
      'ES2022',
      '--module',
      'NodeNext',
      'smoke.ts',
    ],
    { cwd: consumer, stdio: 'inherit' },
  );
  execFileSync(process.execPath, ['smoke.mjs'], { cwd: consumer, stdio: 'inherit' });
  console.log(
    `Built, type-checked, packed and imported all ${contracts.length} binding packages in clean fixtures.`,
  );
} finally {
  if (path.dirname(temporary) !== os.tmpdir()) throw new Error('Unexpected temporary path');
  fs.rmSync(temporary, { recursive: true, force: true });
}
