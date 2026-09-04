/**
 * World props — in-world furniture / scenery (not form widgets).
 */

/** @typedef {'lantern' | 'tree' | 'bench' | 'arch' | 'path_stone' | 'well' | 'banner'} PropKind */

/**
 * @param {PropKind} kind
 * @param {number} x
 * @param {number} y
 * @param {object} [opts]
 */
export function createProp(kind, x, y, opts = {}) {
  return {
    id: opts.id || `prop-${kind}-${Math.round(x)}-${Math.round(y)}`,
    kind,
    x,
    y,
    scale: opts.scale ?? 1,
    z: opts.z ?? y, // painter's algorithm
    interactive: Boolean(opts.interactive),
    label: opts.label || null,
  };
}

/**
 * Default courtyard prop set for night_courtyard biome.
 * @param {{ width: number, height: number }} bounds
 */
export function defaultCourtyardProps(bounds) {
  const w = bounds.width;
  const h = bounds.height;
  const groundY = h * 0.55;
  return [
    createProp('arch', w * 0.5, groundY - 40, { scale: 1.4, id: 'prop-arch-main' }),
    createProp('lantern', w * 0.18, groundY + 20, { scale: 1, id: 'prop-lantern-l' }),
    createProp('lantern', w * 0.82, groundY + 20, { scale: 1, id: 'prop-lantern-r' }),
    createProp('tree', w * 0.08, groundY - 10, { scale: 1.3, id: 'prop-tree-l' }),
    createProp('tree', w * 0.92, groundY - 5, { scale: 1.2, id: 'prop-tree-r' }),
    createProp('tree', w * 0.25, groundY + 80, { scale: 0.9, id: 'prop-tree-fl' }),
    createProp('bench', w * 0.35, groundY + 100, { scale: 1, id: 'prop-bench' }),
    createProp('well', w * 0.72, groundY + 90, { scale: 1, id: 'prop-well' }),
    createProp('banner', w * 0.5, groundY - 100, { scale: 1, id: 'prop-banner', label: 'PERAM' }),
    createProp('path_stone', w * 0.45, groundY + 40, { scale: 1, id: 'prop-stone-1' }),
    createProp('path_stone', w * 0.5, groundY + 70, { scale: 1, id: 'prop-stone-2' }),
    createProp('path_stone', w * 0.55, groundY + 100, { scale: 1, id: 'prop-stone-3' }),
  ];
}

/**
 * @param {Array<object>} props
 * @param {object} prop
 */
export function addProp(props, prop) {
  if (!prop?.id) return props;
  if (props.some((p) => p.id === prop.id)) {
    return props.map((p) => (p.id === prop.id ? { ...p, ...prop } : p));
  }
  return [...props, prop];
}

/**
 * @param {Array<object>} props
 * @param {string} id
 */
export function removeProp(props, id) {
  return props.filter((p) => p.id !== id);
}

/**
 * Props sorted for draw (back to front by z).
 */
export function sortProps(props) {
  return [...props].sort((a, b) => (a.z ?? a.y) - (b.z ?? b.y));
}
