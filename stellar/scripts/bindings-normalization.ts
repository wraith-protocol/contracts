/**
 * The stellar CLI template that generates each binding package ships a Buffer
 * polyfill targeted at DOM runtimes:
 *
 *   if (typeof window !== "undefined") {
 *     //@ts-ignore Buffer exists
 *     window.Buffer = window.Buffer || Buffer;
 *   }
 *
 * It relies on a `@ts-ignore` (failing strict type checks and linting) and is
 * skipped entirely in edge/worker runtimes, where `window` does not exist.
 * This is the exact string it emits, so regeneration can be normalized back
 * deterministically.
 */
export const CLI_BUFFER_SHIM = `if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}`;

/**
 * Typed, runtime-neutral replacement: Node, browsers, and edge isolates all
 * share `globalThis`, and Node already defines `Buffer` there, so the guard
 * only assigns in non-Node runtimes. No `@ts-ignore`, and the assignment
 * targets the one object every runtime exposes — `window` is not consulted.
 * Kept free of TypeScript-only syntax so the semantics can also be executed
 * verbatim in an isolated runtime (see bindings-portability.test.ts).
 */
export const RUNTIME_NEUTRAL_BUFFER_SHIM = `if (typeof Reflect.get(globalThis, "Buffer") === "undefined") {
  Object.assign(globalThis, { Buffer });
}`;

/**
 * Rewrites the stellar CLI's window-targeted Buffer polyfill into the typed,
 * runtime-neutral form. Applied on every generated file so the checked-in
 * bindings stay byte-reproducible with `pnpm bindings:stellar`.
 */
export function normalizeBufferRuntimeSource(content: string): string {
  return content.replaceAll(CLI_BUFFER_SHIM, RUNTIME_NEUTRAL_BUFFER_SHIM);
}

/** Avoid reserved identifiers in generated function-type destructuring.
 * Only the parameter binding changes; public object keys and the spec stay intact.
 */
export function normalizeReservedArguments(content: string): string {
  const reserved = new Set(
    'break case catch class const continue debugger default delete do else enum export extends false finally for function if import in instanceof new null return super switch this throw true try typeof var void while with yield let static implements interface package private protected public await'.split(
      ' ',
    ),
  );
  return content.replace(/\(\{([^{}]*)\}:\s*\{/g, (match, list: string) => {
    const names = list.split(',').map((name) => name.trim().split(':')[0]);
    return names.some((name) => reserved.has(name)) ? '(args: {' : match;
  });
}
