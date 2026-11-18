//! Hybrid logical clock (Kulkarni et al. 2014, Fig. 5).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct HlcWire {
    pub l: u64,
    pub c: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HlcState {
    pub l: u64,
    pub c: u16,
}

/// Physical time in microseconds since Unix epoch (`CLOCK_REALTIME`).
pub fn physical_time_us() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros(),
    )
    .unwrap_or(u64::MAX)
}

pub fn local_append(state: &mut HlcState, pt_us: u64) -> Result<HlcWire, String> {
    let mut pt = pt_us;
    loop {
        let l_prime = state.l;
        state.l = state.l.max(pt);
        if state.l == l_prime {
            match state.c.checked_add(1) {
                Some(c) => state.c = c,
                None => {
                    pt = pt.saturating_add(1);
                    continue;
                }
            }
        } else {
            state.c = 0;
        }
        return Ok(HlcWire {
            l: state.l,
            c: state.c,
        });
    }
}

pub fn receive_merge(state: &mut HlcState, pt_us: u64, remote: HlcWire) -> Result<HlcWire, String> {
    let mut pt = pt_us;
    loop {
        let l_prime = state.l;
        state.l = state.l.max(l_prime).max(remote.l).max(pt);
        let next_c = match (
            state.l == l_prime && state.l == remote.l,
            state.l == l_prime,
            state.l == remote.l,
        ) {
            (true, _, _) => state.c.max(remote.c).checked_add(1),
            (_, true, _) => state.c.checked_add(1),
            (_, _, true) => remote.c.checked_add(1),
            _ => Some(0),
        };
        let Some(next_c) = next_c else {
            pt = pt.saturating_add(1);
            continue;
        };
        state.c = next_c;
        return Ok(HlcWire {
            l: state.l,
            c: state.c,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_append_monotonic_when_clock_steps_back() {
        let mut s = HlcState { l: 5_000_000, c: 2 };
        let first = local_append(&mut s, 4_000_000).unwrap();
        assert_eq!(first.l, 5_000_000);
        assert_eq!(first.c, 3);
        let second = local_append(&mut s, 3_000_000).unwrap();
        assert!(second.l >= first.l);
        assert!(second.c > 0 || second.l > first.l);
    }

    #[test]
    fn receive_merge_beats_both_clocks() {
        let mut s = HlcState { l: 10, c: 1 };
        let out = receive_merge(&mut s, 20, HlcWire { l: 15, c: 3 }).unwrap();
        assert_eq!(out.l, 20);
    }
}
