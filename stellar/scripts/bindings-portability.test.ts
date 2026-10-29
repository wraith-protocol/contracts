/**
 * Runtime-portability smoke tests for the generated Stellar TypeScript
 * bindings.
 *
 * The stellar CLI template used to ship a window-targeted Buffer polyfill
 * annotated with `@ts-ignore`, which fails strict type checks and is skipped
 * entirely in edge runtimes where `window` does not exist. The generator now
 * normalizes that polyfill into a typed, runtime-neutral guard
 * (`normalizeBufferRuntimeSource` in generate-bindings.ts), and these tests
 * pin the resulting behavior:
 *
 * - plain Node import works and leaves the platform Buffer in place
 * - the shim's runtime semantics are verified verbatim in an isolated
 *   edge-style runtime (no window, no pre-existing Buffer): the module
 *   installs its own Buffer polyfill on globalThis, and never overwrites one
 * - a browser bundle builds without Node builtins at bundle time and imports
 *   without overwriting an existing Buffer
 * - the checked-in bindings contain no suppressions and match what the
 *   generator emits, keeping `pnpm bindings:stellar` output reproducible
 */

import * as esbuild from 'esbuild';
import { readFileSync, rmSync, mkdtempSync, existsSync } from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, afterEach, describe, expect, it, vi } from 'vitest';

const SCRIPTS_DIR = path.dirname(fileURLToPath(import.meta.url));
const BINDINGS_ROOT = path.resolve(SCRIPTS_DIR, '..', 'bindings', 'typescript');

const PACKAGES = [
  'stealth-announcer',
  'stealth-registry',
  'stealth-sender',
  'stealth-batch-sender',
  'wraith-names',
] as const;

const nodeBuffer = Buffer;

const bundleWorkdir = path.join(
  os.tmpdir(),
  `wraith-bindings-portability-${process.pid}-${Date.now()}`,
);

// `generate-bindings.ts` runs its CLI entrypoint (`main().catch(... process.exit(1))`)
// as a module side effect. Importing it for the pure helpers must not let that
// entrypoint terminate the Vitest worker, so we guard process.exit while loading.
let CLI_BUFFER_SHIM: string;
let RUNTIME_NEUTRAL_BUFFER_SHIM: string;
let normalizeBufferRuntimeSource: (source: string) => string;

beforeAll(async () => {
  const originalExit = process.exit;
  const exitSpy = vi.fn((code?: number) => undefined as never);
  process.exit = exitSpy as unknown as typeof process.exit;
  try {
    const mod = await import('./generate-bindings.js');
    CLI_BUFFER_SHIM = mod.CLI_BUFFER_SHIM;
    RUNTIME_NEUTRAL_BUFFER_SHIM = mod.RUNTIME_NEUTRAL_BUFFER_SHIM;
    normalizeBufferRuntimeSource = mod.normalizeBufferRuntimeSource;
  } finally {
    process.exit = originalExit;
  }
});

afterAll(() => {
  vi.resetModules();
  if (existsSync(bundleWorkdir)) rmSync(bundleWorkdir, { recursive: true, force: true });
});

describe('Node import', () => {
  it('imports every generated binding package with Client exported', async () => {
    // Literal specifiers: runtime-neutral guards must hold with no window in
    // scope, mirroring edge-style isolation enforced elsewhere.
    const imported = [
      await import('../bindings/typescript/stealth-announcer/src/index.js'),
      await import('../bindings/typescript/stealth-registry/src/index.js'),
      await import('../bindings/typescript/stealth-sender/src/index.js'),
      await import('../bindings/typescript/stealth-batch-sender/src/index.js'),
      await import('../bindings/typescript/wraith-names/src/index.js'),
    ];
    for (const mod of imported) {
      expect(Object.keys(mod).some((name) => name.startsWith('Client'))).toBe(true);
    }
  });

  it('imports the top-level barrel', async () => {
    const barrel = await import('../bindings/typescript/index.js');
    expect(Object.keys(barrel).sort()).toEqual(
      ['StealthAnnouncer', 'StealthBatchSender', 'StealthRegistry', 'StealthSender', 'WraithNames'].sort(),
    );
  });

  it('does not overwrite the platform Buffer on globalThis', async () => {
    await import('../bindings/typescript/stealth-registry/src/index.js');
    expect(globalThis.Buffer).toBe(nodeBuffer);
  });
});

describe('Runtime-neutral Buffer guard (edge-style isolated runtime)', () => {
  // The shim is kept free of TypeScript-only syntax precisely so these tests
  // can execute its emitted code verbatim, pinning semantics at two levels.
  it('installs the model polyfill on globalThis when Buffer is absent', () => {
    const context = vm.createContext({
      // Stand-in for the module's imported Buffer polyfill binding. The
      // sandbox itself starts with no Buffer, like edge isolates.
      polyfill: nodeBuffer,
    });
    const result = vm.runInContext(
      `const Buffer = polyfill;\n${RUNTIME_NEUTRAL_BUFFER_SHIM}\ntypeof globalThis.Buffer;`,
      context,
    );
    expect(result).toBe('function');
    expect((vm.runInContext('globalThis.Buffer', context) as unknown as typeof Buffer)).toBe(nodeBuffer);
    expect(vm.runInContext('typeof window', context)).toBe('undefined');
  });

  it('leaves an existing globalThis.Buffer untouched', () => {
    const context = vm.createContext({ Buffer: nodeBuffer });
    vm.runInContext(RUNTIME_NEUTRAL_BUFFER_SHIM, context);
    const kept = vm.runInContext('globalThis.Buffer', context) as unknown as typeof Buffer;
    expect(kept).toBe(nodeBuffer);
    expect(kept.from('aGVsbG8=', 'base64').toString('utf8')).toBe('hello');
  });
});

describe('Suppression-free bindings stay reproducible', () => {
  it.each(PACKAGES)('%s: no @ts-ignore, typed runtime-neutral Buffer guard only', (pkg) => {
    const source = readFileSync(path.join(BINDINGS_ROOT, pkg, 'src', 'index.ts'), 'utf8');
    expect(source).not.toContain('@ts-ignore');
    expect(source).not.toContain('window.Buffer');
    expect(source).not.toContain(CLI_BUFFER_SHIM);
    expect(source).toContain(RUNTIME_NEUTRAL_BUFFER_SHIM);
  });

  it('normalizeBufferRuntimeSource maps the CLI shim exactly and is idempotent', () => {
    const fixture = `header\n${CLI_BUFFER_SHIM}\n\nexport class Client {}\n`;
    const once = normalizeBufferRuntimeSource(fixture);

    expect(once).toBe(`header\n${RUNTIME_NEUTRAL_BUFFER_SHIM}\n\nexport class Client {}\n`);
    // Running the normalization again must be a no-op, keeping generated
    // output byte-for-byte reproducible across regeneration runs.
    expect(normalizeBufferRuntimeSource(once)).toBe(once);

    // Content without the CLI shim passes through untouched.
    const plain = 'export const noop = 1;\n';
    expect(normalizeBufferRuntimeSource(plain)).toBe(plain);
  });
});

describe('Browser bundle', () => {
  it('bundles a binding for the browser with no Node builtins and imports cleanly', async () => {
    rmSync(bundleWorkdir, { recursive: true, force: true });
    const outfile = path.join(bundleWorkdir, 'stealth-registry.browser.mjs');
    await esbuild.build({
      entryPoints: [path.join(BINDINGS_ROOT, 'stealth-registry', 'src', 'index.ts')],
      bundle: true,
      platform: 'browser',
      format: 'esm',
      outfile,
      logLevel: 'error',
    });

    // Browsers and edge isolates provide `self` alongside `globalThis`; plain
    // Node does not, so the harness supplies the standard alias before an
    // import that exercises the fully bundled SDK surface.
    const harness = globalThis as { self?: unknown };
    if (harness.self === undefined) harness.self = globalThis;

    const mod = await import(outfile);
    expect(mod.Client).toBeTypeOf('function');
    // The module must not clobber a runtime that already provides Buffer.
    expect(globalThis.Buffer).toBe(nodeBuffer);
  });
});
