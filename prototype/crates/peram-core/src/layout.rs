//! Layout primitives (grid pack + soft repulsion).

pub fn pack_grid(count: u32, cols: u32, col_w: f32, row_h: f32) -> Vec<f32> {
    let cols = cols.max(1);
    let mut out = Vec::with_capacity((count as usize) * 2);
    for i in 0..count {
        let col = i % cols;
        let row = i / cols;
        out.push(col as f32 * col_w);
        out.push(row as f32 * row_h);
    }
    out
}

pub fn layout_tick(positions: &[f32], strength: f32) -> Vec<f32> {
    let n = positions.len() / 2;
    if n == 0 {
        return Vec::new();
    }
    let mut out = positions.to_vec();
    let s = strength.clamp(0.0, 2.0);

    for i in 0..n {
        let mut fx = 0.0f32;
        let mut fy = 0.0f32;
        let ix = positions[i * 2];
        let iy = positions[i * 2 + 1];
        for j in 0..n {
            if i == j {
                continue;
            }
            let dx = ix - positions[j * 2];
            let dy = iy - positions[j * 2 + 1];
            let dist2 = dx * dx + dy * dy + 25.0;
            let inv = s * 400.0 / dist2;
            fx += dx * inv;
            fy += dy * inv;
        }
        fx += -ix * 0.002 * s;
        fy += -iy * 0.002 * s;
        out[i * 2] = ix + fx.clamp(-40.0, 40.0);
        out[i * 2 + 1] = iy + fy.clamp(-40.0, 40.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_grid_three() {
        let p = pack_grid(3, 2, 100.0, 50.0);
        assert_eq!(p.len(), 6);
        assert_eq!(p[0], 0.0);
        assert_eq!(p[2], 100.0);
        assert_eq!(p[5], 50.0);
    }
}
