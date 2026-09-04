/* tslint:disable */
/* eslint-disable */
export const memory: WebAssembly.Memory;
export const engine_version: () => [number, number];
export const hash_id: (a: number, b: number) => number;
export const layout_tick: (a: number, b: number, c: number) => [number, number];
export const pack_grid: (a: number, b: number, c: number, d: number) => [number, number];
export const world_bind_graph: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
export const world_draw_buffer: () => [number, number];
export const world_reset: (a: number, b: number) => void;
export const world_tick: (a: number) => void;
export const world_focus_next: () => void;
export const world_focus_prev: () => void;
export const world_set_focus: (a: number) => void;
export const world_focus_slot: () => number;
export const world_entity_count: () => number;
export const world_prop_count: () => number;
export const __wbindgen_externrefs: WebAssembly.Table;
export const __wbindgen_free: (a: number, b: number, c: number) => void;
export const __wbindgen_malloc: (a: number, b: number) => number;
export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
export const __wbindgen_start: () => void;
