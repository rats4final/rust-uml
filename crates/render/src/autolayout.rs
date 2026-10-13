//! Automatic layout: boxes in layers, so lines mostly run one way.
//!
//! A small layered ("Sugiyama-style") layout: break cycles, put each box one
//! layer below the boxes that point at it, order each layer to reduce
//! crossings, then pull each box towards the boxes it is joined to. Good for
//! the few dozen boxes of an imported diagram; not a general graph drawer.

use std::collections::HashSet;

/// Which way edges run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flow {
    #[default]
    Down,
    Up,
    Right,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Between boxes in one layer.
    pub node: f64,
    /// Between layers.
    pub layer: f64,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            node: 60.0,
            layer: 80.0,
        }
    }
}

/// Top-left corners for boxes of the given `sizes` (width, height), with the
/// whole layout starting at (0, 0). `edges` are (from, to) indices; edges
/// with an index out of range are ignored.
pub fn layered(
    sizes: &[(f64, f64)],
    edges: &[(usize, usize)],
    flow: Flow,
    gap: Spacing,
) -> Vec<(f64, f64)> {
    let n = sizes.len();
    if n == 0 {
        return Vec::new();
    }
    let across = matches!(flow, Flow::Right | Flow::Left);
    // Work as if flowing down: `w` runs along a layer, `h` across layers.
    let dims: Vec<(f64, f64)> = sizes
        .iter()
        .map(|&(w, h)| if across { (h, w) } else { (w, h) })
        .collect();

    let mut seen = HashSet::new();
    let edges: Vec<(usize, usize)> = edges
        .iter()
        .copied()
        .filter(|&(a, b)| a < n && b < n && a != b && seen.insert((a, b)))
        .collect();
    let edges = acyclic(n, &edges);
    let rank = ranks(n, &edges);
    let layers = order(n, &edges, &rank);

    // Positions along each layer, then pulled towards neighbours.
    let mut x = vec![0.0; n];
    for layer in &layers {
        let mut at = 0.0;
        for &v in layer {
            x[v] = at;
            at += dims[v].0 + gap.node;
        }
    }
    let up: Vec<Vec<usize>> = (0..n)
        .map(|v| edges.iter().filter(|e| e.1 == v).map(|e| e.0).collect())
        .collect();
    let down: Vec<Vec<usize>> = (0..n)
        .map(|v| edges.iter().filter(|e| e.0 == v).map(|e| e.1).collect())
        .collect();
    for pass in 0..4 {
        let (seq, neighbours): (Vec<&Vec<usize>>, &Vec<Vec<usize>>) = if pass % 2 == 0 {
            (layers.iter().skip(1).collect(), &up)
        } else {
            (layers.iter().rev().skip(1).collect(), &down)
        };
        for layer in seq {
            place(layer, &mut x, &dims, neighbours, gap.node);
        }
    }
    let min = (0..n).map(|v| x[v]).fold(f64::INFINITY, f64::min);

    // Layers stack; boxes are centred in their layer's depth.
    let depth: Vec<f64> = layers
        .iter()
        .map(|l| l.iter().map(|&v| dims[v].1).fold(0.0, f64::max))
        .collect();
    let mut y = vec![0.0; n];
    let mut top = 0.0;
    for (i, layer) in layers.iter().enumerate() {
        for &v in layer {
            y[v] = top + (depth[i] - dims[v].1) / 2.0;
        }
        top += depth[i] + gap.layer;
    }
    let total = top - gap.layer;

    (0..n)
        .map(|v| {
            let along = x[v] - min;
            let (ax, ay) = match flow {
                Flow::Down => (along, y[v]),
                Flow::Up => (along, total - y[v] - dims[v].1),
                Flow::Right => (y[v], along),
                Flow::Left => (total - y[v] - dims[v].1, along),
            };
            (ax.round(), ay.round())
        })
        .collect()
}

/// Reverse the edges that close a cycle (found by depth-first search, in
/// node order so the result is stable).
fn acyclic(n: usize, edges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut state = vec![0u8; n]; // 0 new, 1 on stack, 2 done
    let mut back = HashSet::new();
    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        state[start] = 1;
        while let Some(&mut (v, ref mut i)) = stack.last_mut() {
            let out: Vec<(usize, usize)> = edges.iter().copied().filter(|e| e.0 == v).collect();
            if let Some(&(_, w)) = out.get(*i) {
                *i += 1;
                match state[w] {
                    0 => {
                        state[w] = 1;
                        stack.push((w, 0));
                    }
                    1 => {
                        back.insert((v, w));
                    }
                    _ => {}
                }
            } else {
                state[v] = 2;
                stack.pop();
            }
        }
    }
    let mut seen = HashSet::new();
    edges
        .iter()
        .map(|&(a, b)| {
            if back.contains(&(a, b)) {
                (b, a)
            } else {
                (a, b)
            }
        })
        .filter(|e| seen.insert(*e))
        .collect()
}

/// Longest path from the sources, then sources moved down next to their
/// highest child so they don't all crowd the top layer.
fn ranks(n: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let mut indeg = vec![0usize; n];
    for &(_, b) in edges {
        indeg[b] += 1;
    }
    let mut rank = vec![0usize; n];
    let mut queue: Vec<usize> = (0..n).filter(|&v| indeg[v] == 0).collect();
    let mut i = 0;
    while let Some(&v) = queue.get(i) {
        i += 1;
        for &(a, b) in edges {
            if a == v {
                rank[b] = rank[b].max(rank[v] + 1);
                indeg[b] -= 1;
                if indeg[b] == 0 {
                    queue.push(b);
                }
            }
        }
    }
    for v in 0..n {
        let has_parent = edges.iter().any(|e| e.1 == v);
        let lowest_child = edges.iter().filter(|e| e.0 == v).map(|e| rank[e.1]).min();
        if let (false, Some(c)) = (has_parent, lowest_child)
            && c > 0
        {
            rank[v] = c - 1;
        }
    }
    rank
}

/// Group by rank and order each layer by the average position of its
/// neighbours (a few sweeps down and up).
fn order(n: usize, edges: &[(usize, usize)], rank: &[usize]) -> Vec<Vec<usize>> {
    let layers_n = rank.iter().copied().max().unwrap_or(0) + 1;
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); layers_n];
    for v in 0..n {
        layers[rank[v]].push(v);
    }
    let mut pos = vec![0.0; n];
    let refresh = |layers: &Vec<Vec<usize>>, pos: &mut Vec<f64>| {
        for l in layers {
            for (i, &v) in l.iter().enumerate() {
                pos[v] = (i as f64 + 0.5) / l.len() as f64;
            }
        }
    };
    refresh(&layers, &mut pos);
    for sweep in 0..6 {
        let downward = sweep % 2 == 0;
        let ids: Vec<usize> = if downward {
            (1..layers_n).collect()
        } else {
            (0..layers_n.saturating_sub(1)).rev().collect()
        };
        for li in ids {
            let mut keyed: Vec<(f64, usize)> = layers[li]
                .iter()
                .map(|&v| {
                    let ns: Vec<f64> = edges
                        .iter()
                        .filter_map(|&(a, b)| {
                            let other = if b == v {
                                a
                            } else if a == v {
                                b
                            } else {
                                return None;
                            };
                            let toward = if downward {
                                rank[other] < rank[v]
                            } else {
                                rank[other] > rank[v]
                            };
                            toward.then_some(pos[other])
                        })
                        .collect();
                    let key = if ns.is_empty() {
                        pos[v]
                    } else {
                        ns.iter().sum::<f64>() / ns.len() as f64
                    };
                    (key, v)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
            layers[li] = keyed.into_iter().map(|(_, v)| v).collect();
            refresh(&layers, &mut pos);
        }
    }
    layers
}

/// Move a layer's boxes towards the centres of their `neighbours`, keeping
/// their order and at least `gap` between them.
fn place(layer: &[usize], x: &mut [f64], dims: &[(f64, f64)], neighbours: &[Vec<usize>], gap: f64) {
    let centre = |v: usize, x: &[f64]| x[v] + dims[v].0 / 2.0;
    let wanted: Vec<f64> = layer
        .iter()
        .map(|&v| {
            let ns = &neighbours[v];
            if ns.is_empty() {
                centre(v, x)
            } else {
                ns.iter().map(|&u| centre(u, x)).sum::<f64>() / ns.len() as f64
            }
        })
        .collect();
    // Left to right at the wanted spot or just right of the previous box…
    let mut at = f64::NEG_INFINITY;
    let mut placed = Vec::with_capacity(layer.len());
    for (i, &v) in layer.iter().enumerate() {
        let left = (wanted[i] - dims[v].0 / 2.0).max(at);
        placed.push(left);
        at = left + dims[v].0 + gap;
    }
    // …then the whole layer shifted so it is, on average, where it wants to be.
    let drift: f64 = layer
        .iter()
        .enumerate()
        .map(|(i, &v)| wanted[i] - (placed[i] + dims[v].0 / 2.0))
        .sum::<f64>()
        / layer.len().max(1) as f64;
    for (i, &v) in layer.iter().enumerate() {
        x[v] = placed[i] + drift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(p: &[(f64, f64)], s: &[(f64, f64)]) -> bool {
        for i in 0..p.len() {
            for j in i + 1..p.len() {
                let (a, b) = ((p[i], s[i]), (p[j], s[j]));
                let sep_x = a.0.0 + a.1.0 <= b.0.0 || b.0.0 + b.1.0 <= a.0.0;
                let sep_y = a.0.1 + a.1.1 <= b.0.1 || b.0.1 + b.1.1 <= a.0.1;
                if !sep_x && !sep_y {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn a_tree_flows_down_without_overlaps() {
        // 0 is the parent of 1, 2, 3; 1 is the parent of 4.
        let sizes = vec![
            (120.0, 60.0),
            (100.0, 80.0),
            (140.0, 40.0),
            (90.0, 50.0),
            (100.0, 50.0),
        ];
        let edges = [(0, 1), (0, 2), (0, 3), (1, 4)];
        let p = layered(&sizes, &edges, Flow::Down, Spacing::default());
        assert!(!overlaps(&p, &sizes));
        assert!(p[0].1 < p[1].1 && p[1].1 < p[4].1);
        assert_eq!(
            p[1].1 + 40.0,
            p[2].1 + 20.0,
            "boxes are centred in their layer"
        );
        // The parent sits over its children.
        let centre = |i: usize| p[i].0 + sizes[i].0 / 2.0;
        assert!(centre(0) > centre(1).min(centre(3)) && centre(0) < centre(1).max(centre(3)));
        assert_eq!(p.iter().map(|q| q.0).fold(f64::INFINITY, f64::min), 0.0);
    }

    #[test]
    fn cycles_other_flows_and_junk_edges() {
        let sizes = vec![(100.0, 50.0); 4];
        let edges = [(0, 1), (1, 2), (2, 0), (2, 3), (3, 3), (9, 1), (0, 1)];
        for flow in [Flow::Down, Flow::Up, Flow::Right, Flow::Left] {
            let p = layered(&sizes, &edges, flow, Spacing::default());
            assert_eq!(p.len(), 4);
            assert!(!overlaps(&p, &sizes), "{flow:?}");
        }
        let right = layered(&sizes, &[(0, 1)], Flow::Right, Spacing::default());
        assert!(right[0].0 < right[1].0 && right[0].1 == right[1].1);
        let up = layered(&sizes, &[(0, 1)], Flow::Up, Spacing::default());
        assert!(up[0].1 > up[1].1);
        assert!(layered(&[], &[], Flow::Down, Spacing::default()).is_empty());
    }

    #[test]
    fn unconnected_boxes_share_a_row() {
        let sizes = vec![(100.0, 50.0); 3];
        let p = layered(&sizes, &[], Flow::Down, Spacing::default());
        assert!(p.iter().all(|q| q.1 == 0.0));
        assert!(!overlaps(&p, &sizes));
    }
}
