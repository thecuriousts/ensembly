/* tslint:disable */
/* eslint-disable */

/**
 * Engine banner for feature detect / desktop parity.
 */
export function engine_version(): string;

export function hash_id(id: string): number;

export function layout_tick(positions: Float32Array, strength: number): Float32Array;

export function pack_grid(count: number, cols: number, col_w: number, row_h: number): Float32Array;

/**
 * Bind graph nodes.
 * `types_joined` — types separated by `\n`
 * `realms_joined` — realms separated by `\n` (same length)
 * `ids_joined` — ids separated by `\n`
 * `hitl_mask` — bit i set if node i is HITL (as u32, first 32 nodes)
 */
export function world_bind_graph(types_joined: string, realms_joined: string, ids_joined: string, hitl_mask: number): void;

/**
 * Flattened draw buffer — see world::World::draw_buffer docs.
 */
export function world_draw_buffer(): Float32Array;

export function world_entity_count(): number;

export function world_focus_next(): void;

export function world_focus_prev(): void;

export function world_focus_slot(): number;

export function world_prop_count(): number;

/**
 * Reset world to empty courtyard with avatar + default props.
 */
export function world_reset(width: number, height: number): void;

export function world_set_focus(slot: number): void;

export function world_tick(speed: number): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly engine_version: () => [number, number];
    readonly hash_id: (a: number, b: number) => number;
    readonly layout_tick: (a: number, b: number, c: number) => [number, number];
    readonly pack_grid: (a: number, b: number, c: number, d: number) => [number, number];
    readonly world_bind_graph: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly world_draw_buffer: () => [number, number];
    readonly world_reset: (a: number, b: number) => void;
    readonly world_tick: (a: number) => void;
    readonly world_focus_next: () => void;
    readonly world_focus_prev: () => void;
    readonly world_set_focus: (a: number) => void;
    readonly world_focus_slot: () => number;
    readonly world_entity_count: () => number;
    readonly world_prop_count: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
