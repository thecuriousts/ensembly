//! Shared world simulation — browser (WASM) and future desktop (native).
//! Inspired by Rust+WASM engine practice (in-browser world sim, thin host shell).
//! Agent/world-model trend: sim state lives in a controlled engine; hosts only present.

#![allow(dead_code)]

/// Entity kind codes (stable ABI for hosts).
pub const KIND_AVATAR: u8 = 1;
pub const KIND_BEACON_DIGITAL: u8 = 2;
pub const KIND_BEACON_PHYSICAL: u8 = 3;
pub const KIND_BEACON_HITL: u8 = 4;
pub const KIND_BEACON_PHASE: u8 = 5;
pub const KIND_AGENT: u8 = 6;
pub const KIND_PROP: u8 = 10;

/// Prop sub-kinds
pub const PROP_TREE: u8 = 1;
pub const PROP_LANTERN: u8 = 2;
pub const PROP_BENCH: u8 = 3;
pub const PROP_ARCH: u8 = 4;
pub const PROP_WELL: u8 = 5;
pub const PROP_BANNER: u8 = 6;
pub const PROP_STONE: u8 = 7;

/// flags: bit0 focused, bit1 selected, bit2 hitl
pub const FLAG_FOCUSED: u8 = 1;
pub const FLAG_SELECTED: u8 = 2;
pub const FLAG_HITL: u8 = 4;

#[derive(Clone, Debug)]
pub struct Entity {
    pub id_hash: u32,
    pub kind: u8,
    pub x: f32,
    pub y: f32,
    pub graph_slot: i32, // index into bound graph nodes, -1 if none
    pub flags: u8,
    pub z: f32,
}

#[derive(Clone, Debug)]
pub struct Prop {
    pub kind: u8,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[derive(Clone, Debug)]
pub struct World {
    pub width: f32,
    pub height: f32,
    pub time: u32,
    pub focus_slot: i32,
    pub entities: Vec<Entity>,
    pub props: Vec<Prop>,
    /// Parallel label hashes for beacons (graph slot order)
    pub beacon_hashes: Vec<u32>,
    pub beacon_kinds: Vec<u8>,
}

impl Default for World {
    fn default() -> Self {
        Self::new_courtyard(1600.0, 900.0)
    }
}

impl World {
    pub fn new_courtyard(width: f32, height: f32) -> Self {
        let mut w = World {
            width,
            height,
            time: 0,
            focus_slot: 0,
            entities: Vec::new(),
            props: courtyard_props(width, height),
            beacon_hashes: Vec::new(),
            beacon_kinds: Vec::new(),
        };
        // Avatar at path foot
        w.entities.push(Entity {
            id_hash: fnv1a(b"avatar-player"),
            kind: KIND_AVATAR,
            x: width * 0.5,
            y: height * 0.75,
            graph_slot: -1,
            flags: 0,
            z: height * 0.75,
        });
        w
    }

    /// Bind graph nodes as beacons along courtyard path.
    /// kinds: parallel kind codes for each node
    pub fn bind_beacons(&mut self, kinds: &[u8], hashes: &[u32]) {
        // drop old beacons
        self.entities.retain(|e| e.kind == KIND_AVATAR);
        self.beacon_kinds = kinds.to_vec();
        self.beacon_hashes = hashes.to_vec();

        let n = kinds.len().max(1);
        let base_y = self.height * 0.58;
        let start_x = self.width * 0.22;
        let span = self.width * 0.56;

        for (i, &k) in kinds.iter().enumerate() {
            let t = if kinds.len() == 1 {
                0.5
            } else {
                i as f32 / (n - 1) as f32
            };
            let x = start_x + span * t;
            let y = base_y + (t * std::f32::consts::PI).sin() * 40.0 + ((i % 2) as f32) * 18.0;
            let hash = hashes.get(i).copied().unwrap_or(i as u32);
            self.entities.push(Entity {
                id_hash: hash,
                kind: k,
                x,
                y,
                graph_slot: i as i32,
                flags: if k == KIND_BEACON_HITL { FLAG_HITL } else { 0 },
                z: y,
            });
        }
        self.focus_slot = 0;
        self.apply_focus_flags();
    }

    pub fn set_focus_slot(&mut self, slot: i32) {
        if slot < 0 {
            self.focus_slot = 0;
        } else if (slot as usize) >= self.beacon_kinds.len() && !self.beacon_kinds.is_empty() {
            self.focus_slot = (self.beacon_kinds.len() - 1) as i32;
        } else {
            self.focus_slot = slot;
        }
        self.apply_focus_flags();
    }

    pub fn focus_next(&mut self) {
        if self.beacon_kinds.is_empty() {
            return;
        }
        let n = self.beacon_kinds.len() as i32;
        self.focus_slot = (self.focus_slot + 1).rem_euclid(n);
        self.apply_focus_flags();
    }

    pub fn focus_prev(&mut self) {
        if self.beacon_kinds.is_empty() {
            return;
        }
        let n = self.beacon_kinds.len() as i32;
        self.focus_slot = (self.focus_slot - 1).rem_euclid(n);
        self.apply_focus_flags();
    }

    fn apply_focus_flags(&mut self) {
        for e in &mut self.entities {
            if e.graph_slot >= 0 {
                if e.graph_slot == self.focus_slot {
                    e.flags |= FLAG_FOCUSED | FLAG_SELECTED;
                } else {
                    e.flags &= !(FLAG_FOCUSED | FLAG_SELECTED);
                    if e.kind == KIND_BEACON_HITL {
                        e.flags |= FLAG_HITL;
                    }
                }
            }
        }
    }

    /// One simulation tick: avatar seeks focused beacon.
    pub fn tick(&mut self, speed: f32) {
        self.time = self.time.wrapping_add(1);
        let target = self
            .entities
            .iter()
            .find(|e| e.graph_slot == self.focus_slot && e.kind != KIND_AVATAR)
            .map(|e| (e.x, e.y + 36.0));

        if let Some((tx, ty)) = target {
            if let Some(av) = self.entities.iter_mut().find(|e| e.kind == KIND_AVATAR) {
                seek(av, tx, ty, speed);
            }
        }
        // z-sort entities by y for draw stability (in-place)
        self.entities
            .sort_by(|a, b| a.z.partial_cmp(&b.z).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// Draw buffer ABI (host-agnostic):
    /// [time, width, height, focus_slot, ent_count, prop_count,
    ///  then entities: kind, flags, x, y, graph_slot (as f32), id_hash (as f32)  × N
    ///  then props: kind, x, y, scale × M]
    pub fn draw_buffer(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(8 + self.entities.len() * 6 + self.props.len() * 4);
        out.push(self.time as f32);
        out.push(self.width);
        out.push(self.height);
        out.push(self.focus_slot as f32);
        out.push(self.entities.len() as f32);
        out.push(self.props.len() as f32);
        for e in &self.entities {
            out.push(e.kind as f32);
            out.push(e.flags as f32);
            out.push(e.x);
            out.push(e.y);
            out.push(e.graph_slot as f32);
            out.push(e.id_hash as f32);
        }
        for p in &self.props {
            out.push(p.kind as f32);
            out.push(p.x);
            out.push(p.y);
            out.push(p.scale);
        }
        out
    }

    pub fn entity_count(&self) -> u32 {
        self.entities.len() as u32
    }

    pub fn prop_count(&self) -> u32 {
        self.props.len() as u32
    }
}

fn seek(e: &mut Entity, tx: f32, ty: f32, speed: f32) {
    let dx = tx - e.x;
    let dy = ty - e.y;
    let dist = (dx * dx + dy * dy).sqrt().max(0.0001);
    if dist <= speed {
        e.x = tx;
        e.y = ty;
        e.z = ty;
        return;
    }
    e.x += dx / dist * speed;
    e.y += dy / dist * speed;
    e.z = e.y;
}

fn courtyard_props(w: f32, h: f32) -> Vec<Prop> {
    let gy = h * 0.55;
    vec![
        Prop { kind: PROP_ARCH, x: w * 0.5, y: gy - 40.0, scale: 1.4 },
        Prop { kind: PROP_LANTERN, x: w * 0.18, y: gy + 20.0, scale: 1.0 },
        Prop { kind: PROP_LANTERN, x: w * 0.82, y: gy + 20.0, scale: 1.0 },
        Prop { kind: PROP_TREE, x: w * 0.08, y: gy - 10.0, scale: 1.3 },
        Prop { kind: PROP_TREE, x: w * 0.92, y: gy - 5.0, scale: 1.2 },
        Prop { kind: PROP_TREE, x: w * 0.25, y: gy + 80.0, scale: 0.9 },
        Prop { kind: PROP_BENCH, x: w * 0.35, y: gy + 100.0, scale: 1.0 },
        Prop { kind: PROP_WELL, x: w * 0.72, y: gy + 90.0, scale: 1.0 },
        Prop { kind: PROP_BANNER, x: w * 0.5, y: gy - 100.0, scale: 1.0 },
        Prop { kind: PROP_STONE, x: w * 0.45, y: gy + 40.0, scale: 1.0 },
        Prop { kind: PROP_STONE, x: w * 0.5, y: gy + 70.0, scale: 1.0 },
        Prop { kind: PROP_STONE, x: w * 0.55, y: gy + 100.0, scale: 1.0 },
    ]
}

pub fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in bytes {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Map host graph type string → kind code.
pub fn kind_from_type(t: &str, realm: &str, hitl: bool) -> u8 {
    if hitl || t == "hitl" {
        return KIND_BEACON_HITL;
    }
    if t == "physical" || realm == "physical" {
        return KIND_BEACON_PHYSICAL;
    }
    if t == "phase" {
        return KIND_BEACON_PHASE;
    }
    if t == "game" {
        return KIND_AGENT;
    }
    KIND_BEACON_DIGITAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courtyard_has_props_and_avatar() {
        let w = World::new_courtyard(1600.0, 900.0);
        assert!(w.props.len() >= 8);
        assert_eq!(w.entities.iter().filter(|e| e.kind == KIND_AVATAR).count(), 1);
    }

    #[test]
    fn bind_and_seek() {
        let mut w = World::new_courtyard(1600.0, 900.0);
        w.bind_beacons(
            &[KIND_BEACON_DIGITAL, KIND_BEACON_PHYSICAL, KIND_BEACON_HITL],
            &[1, 2, 3],
        );
        assert_eq!(w.entities.len(), 4); // avatar + 3
        w.set_focus_slot(2);
        let av_before = w.entities.iter().find(|e| e.kind == KIND_AVATAR).unwrap().x;
        for _ in 0..30 {
            w.tick(20.0);
        }
        let av = w.entities.iter().find(|e| e.kind == KIND_AVATAR).unwrap();
        let target = w.entities.iter().find(|e| e.graph_slot == 2).unwrap();
        assert!((av.y - (target.y + 36.0)).abs() < 25.0 || (av.x - av_before).abs() > 1.0);
        let buf = w.draw_buffer();
        assert!(buf.len() > 10);
        assert_eq!(buf[4] as usize, w.entities.len());
    }

    #[test]
    fn focus_wraps() {
        let mut w = World::new_courtyard(1600.0, 900.0);
        w.bind_beacons(&[KIND_BEACON_DIGITAL, KIND_BEACON_PHASE], &[9, 8]);
        w.focus_next();
        assert_eq!(w.focus_slot, 1);
        w.focus_next();
        assert_eq!(w.focus_slot, 0);
    }
}
