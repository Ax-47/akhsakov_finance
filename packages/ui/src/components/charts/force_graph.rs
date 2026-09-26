//! Interactive force-directed graph. Every pair of nodes is joined by a
//! spring whose rest length comes from their correlation, so related stocks
//! pull together and unrelated / opposite ones push apart. Nodes can be
//! dragged; on release the springs pull the graph back into balance.

use crate::components::color_schema::CHART_COLORS_HEX;
use dioxus::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use types::ticker_symbol::TickerSymbol;

static GRAPH_CTR: AtomicUsize = AtomicUsize::new(0);

pub const VIEW_W: f64 = 600.0;
pub const VIEW_H: f64 = 380.0;
const PADDING: f64 = 48.0;

/// Pairs weaker than this get no visible edge (their spring still acts).
pub const EDGE_THRESHOLD: f64 = 0.3;

// Physics tuning, per animation frame.
const SPRING: f64 = 0.08;
const COLLIDE: f64 = 0.5;
const CENTER_PULL: f64 = 0.01;
const DAMPING: f64 = 0.55;
const ALPHA_DECAY: f64 = 0.97;
const ALPHA_MIN: f64 = 0.01;
const DRAG_ALPHA: f64 = 0.5;

/// Resolves on every animation frame; the Rust side answers `true` to keep
/// going or `false` to stop, so the loop only runs while the graph moves.
const FRAME_LOOP_JS: &str = r#"
    while (true) {
        await new Promise((r) => requestAnimationFrame(r));
        dioxus.send(true);
        if (!(await dioxus.recv())) break;
    }
"#;

// Pointer messages from POINTER_JS: `[kind, node, x, y]`.
const POINTER_DOWN: u8 = 0;
const POINTER_MOVE: u8 = 1;

/// Native pointer listeners on the SVG, attached once per graph. They run
/// synchronously in the webview (no round trip before a drag starts, no
/// reliance on `MouseEvent.buttons`, which WebKitGTK reports unreliably),
/// capture the pointer so drags continue outside the SVG, and convert to
/// view-box units with the SVG's screen matrix.
const POINTER_JS: &str = r#"
    const id = await dioxus.recv();
    let svg;
    while (!(svg = document.getElementById(id))) {
        await new Promise((r) => requestAnimationFrame(r));
    }
    const toView = (e) => {
        const p = new DOMPoint(e.clientX, e.clientY)
            .matrixTransform(svg.getScreenCTM().inverse());
        return [p.x, p.y];
    };
    let active = null;
    const end = () => {
        if (active === null) return;
        active = null;
        dioxus.send([2, 0, 0, 0]);
    };
    svg.addEventListener("pointerdown", (e) => {
        const node = e.target.closest("[data-node]");
        if (!node || e.button !== 0) return;
        e.preventDefault();
        active = e.pointerId;
        try { svg.setPointerCapture(e.pointerId); } catch (_) {}
        dioxus.send([0, Number(node.getAttribute("data-node")), ...toView(e)]);
    });
    svg.addEventListener("pointermove", (e) => {
        if (e.pointerId !== active) return;
        e.preventDefault();
        dioxus.send([1, 0, ...toView(e)]);
    });
    svg.addEventListener("pointerup", end);
    svg.addEventListener("pointercancel", end);
    svg.addEventListener("lostpointercapture", end);
    await new Promise(() => {});
"#;

#[component]
pub fn ForceGraph(
    tickers: Vec<TickerSymbol>,
    corr: ReadSignal<Vec<Vec<f64>>>,
    /// Portfolio weight (%) per ticker; sets the node size.
    weights: ReadSignal<Vec<f64>>,
    hovered: Signal<Option<usize>>,
) -> Element {
    let mut sim = use_signal(Sim::default);
    let running = use_signal(|| false);
    let graph_no = use_hook(|| GRAPH_CTR.fetch_add(1, Ordering::Relaxed));

    let radii = use_memo(move || {
        weights
            .read()
            .iter()
            .map(|w| node_radius(*w))
            .collect::<Vec<_>>()
    });
    let targets = use_memo(move || spring_lengths(&corr.read()));

    // Drag handling: a single listener script for the component's lifetime.
    use_future(move || async move {
        let mut pointer = document::eval(POINTER_JS);
        if pointer.send(format!("force-graph-{graph_no}")).is_err() {
            return;
        }
        while let Ok((kind, node, x, y)) = pointer.recv::<(u8, usize, f64, f64)>().await {
            match kind {
                POINTER_DOWN => {
                    sim.write().dragged = Some(node);
                    hovered.set(Some(node));
                    animate(sim, running, targets, radii);
                }
                POINTER_MOVE => {}
                _ => {
                    sim.write().dragged = None;
                    continue;
                }
            }
            let Some(i) = sim.peek().dragged else {
                continue;
            };
            let r = radii.peek().get(i).copied().unwrap_or(14.0);
            sim.write().drag_to(i, x, y, r);
        }
    });

    // New data: start from the stress layout, contracted toward the centre,
    // so nodes spring outward into place.
    use_effect(move || {
        let bodies = layout(&corr.read())
            .into_iter()
            .map(|(x, y)| Body::at(lerp(VIEW_W / 2.0, x, 0.2), lerp(VIEW_H / 2.0, y, 0.2)))
            .collect();
        sim.set(Sim {
            bodies,
            alpha: 1.0,
            dragged: None,
        });
        animate(sim, running, targets, radii);
    });

    let corr_now = corr.read();
    let state = sim.read();
    let edges = edges(&corr_now);

    rsx! {
        svg {
            class: "w-full lg:flex-1 select-none",
            style: "touch-action:none;",
            view_box: "0 0 {VIEW_W} {VIEW_H}",
            id: "force-graph-{graph_no}",
            onpointerleave: move |_| {
                if sim.peek().dragged.is_none() {
                    hovered.set(None);
                }
            },

            if state.bodies.len() == corr_now.len() {
                for (i, j, c) in edges {
                    GraphEdge {
                        key: "{i}-{j}",
                        from: state.bodies[i].pos(),
                        to: state.bodies[j].pos(),
                        corr: c,
                        state: edge_state(hovered(), i, j),
                    }
                }
                for (i, ticker) in tickers.iter().enumerate() {
                    g {
                        key: "{ticker}",
                        "data-node": "{i}",
                        class: if state.dragged == Some(i) { "cursor-grabbing" } else { "cursor-grab" },
                        style: "transition:opacity .2s;",
                        opacity: if node_dimmed(hovered(), i, &corr_now) { "0.3" } else { "1" },
                        onpointerenter: move |_| {
                            if sim.peek().dragged.is_none() {
                                hovered.set(Some(i));
                            }
                        },
                        GraphNode {
                            ticker: ticker.to_string(),
                            at: state.bodies[i].pos(),
                            radius: radii.read().get(i).copied().unwrap_or(14.0),
                            weight: weights.read().get(i).copied().unwrap_or(0.0),
                            color: CHART_COLORS_HEX[i % CHART_COLORS_HEX.len()],
                            dragging: state.dragged == Some(i),
                        }
                    }
                }
            }
        }
    }
}

/// Drives the simulation on animation frames until it settles. No-op if a
/// loop is already running.
fn animate(
    mut sim: Signal<Sim>,
    mut running: Signal<bool>,
    targets: Memo<Vec<Vec<f64>>>,
    radii: Memo<Vec<f64>>,
) {
    if *running.peek() {
        return;
    }
    running.set(true);
    spawn(async move {
        let mut frames = document::eval(FRAME_LOOP_JS);
        while frames.recv::<bool>().await.is_ok() {
            let busy = sim.with_mut(|s| s.step(&targets.peek(), &radii.peek()));
            if frames.send(busy).is_err() || !busy {
                break;
            }
        }
        running.set(false);
    });
}

// ─── Rendering ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum EdgeState {
    Normal,
    Highlighted,
    Dimmed,
}

#[component]
fn GraphEdge(from: (f64, f64), to: (f64, f64), corr: f64, state: EdgeState) -> Element {
    let color = if corr >= 0.0 { "#a6e3a1" } else { "#f38ba8" };
    let width = 1.0 + corr.abs() * 4.0;
    let opacity = match state {
        EdgeState::Normal => 0.15 + corr.abs() * 0.5,
        EdgeState::Highlighted => 0.9,
        EdgeState::Dimmed => 0.05,
    };
    let (mx, my) = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    rsx! {
        line {
            x1: "{from.0:.1}", y1: "{from.1:.1}", x2: "{to.0:.1}", y2: "{to.1:.1}",
            stroke: color,
            stroke_width: "{width:.1}",
            stroke_dasharray: if corr < 0.0 { "6 4" } else { "none" },
            stroke_linecap: "round",
            opacity: "{opacity:.2}",
            style: "transition:opacity .2s;",
        }
        if state == EdgeState::Highlighted {
            text {
                x: "{mx:.1}", y: "{my:.1}",
                dy: "-4",
                text_anchor: "middle",
                font_size: "11",
                font_weight: "700",
                fill: color,
                paint_order: "stroke",
                stroke: "#1e1e2e",
                stroke_width: "3",
                "{corr:+.2}"
            }
        }
    }
}

#[component]
fn GraphNode(
    ticker: String,
    at: (f64, f64),
    radius: f64,
    weight: f64,
    color: String,
    dragging: bool,
) -> Element {
    rsx! {
        circle {
            cx: "{at.0:.1}", cy: "{at.1:.1}", r: "{radius:.1}",
            fill: "{color}",
            fill_opacity: if dragging { "0.45" } else { "0.2" },
            stroke: "{color}",
            stroke_width: if dragging { "3" } else { "2" },
        }
        text {
            x: "{at.0:.1}", y: "{at.1:.1}",
            dy: "4",
            text_anchor: "middle",
            font_size: "12",
            font_weight: "700",
            fill: "#cdd6f4",
            pointer_events: "none",
            "{ticker}"
        }
        title { "{ticker} · {weight:.1}% of portfolio" }
    }
}

fn edges(corr: &[Vec<f64>]) -> Vec<(usize, usize, f64)> {
    let n = corr.len();
    (0..n)
        .flat_map(|i| ((i + 1)..n).map(move |j| (i, j)))
        .map(|(i, j)| (i, j, corr[i][j]))
        .filter(|(_, _, c)| c.abs() >= EDGE_THRESHOLD)
        .collect()
}

fn edge_state(hovered: Option<usize>, i: usize, j: usize) -> EdgeState {
    match hovered {
        None => EdgeState::Normal,
        Some(h) if h == i || h == j => EdgeState::Highlighted,
        Some(_) => EdgeState::Dimmed,
    }
}

/// While hovering, fade stocks that aren't meaningfully related to it.
fn node_dimmed(hovered: Option<usize>, i: usize, corr: &[Vec<f64>]) -> bool {
    hovered.is_some_and(|h| h != i && corr[h][i].abs() < EDGE_THRESHOLD)
}

fn node_radius(weight: f64) -> f64 {
    14.0 + weight.max(0.0).sqrt() * 2.5
}

// ─── Physics ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Body {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
}

impl Body {
    fn at(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            ..Self::default()
        }
    }

    fn pos(&self) -> (f64, f64) {
        (self.x, self.y)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Sim {
    bodies: Vec<Body>,
    /// "Temperature": scales forces and decays each frame so motion settles.
    alpha: f64,
    dragged: Option<usize>,
}

impl Sim {
    /// Advances one frame. Returns whether the graph is still moving.
    fn step(&mut self, targets: &[Vec<f64>], radii: &[f64]) -> bool {
        let n = self.bodies.len();
        if n != targets.len() || n != radii.len() {
            return false;
        }

        let k = SPRING / (n.max(2) - 1) as f64;
        let mut force = vec![(0.0, 0.0); n];
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.bodies[i], self.bodies[j]);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let dist = dx.hypot(dy).max(0.01);
                // Positive pulls the pair together, negative pushes apart.
                let mut f = k * (dist - targets[i][j]);
                let min_gap = radii[i] + radii[j] + 6.0;
                if dist < min_gap {
                    f -= COLLIDE * (min_gap - dist);
                }
                let (fx, fy) = (dx / dist * f, dy / dist * f);
                force[i].0 += fx;
                force[i].1 += fy;
                force[j].0 -= fx;
                force[j].1 -= fy;
            }
        }

        for (i, body) in self.bodies.iter_mut().enumerate() {
            if self.dragged == Some(i) {
                body.vx = 0.0;
                body.vy = 0.0;
                continue;
            }
            let fx = force[i].0 + (VIEW_W / 2.0 - body.x) * CENTER_PULL;
            let fy = force[i].1 + (VIEW_H / 2.0 - body.y) * CENTER_PULL;
            body.vx = (body.vx + fx * self.alpha) * DAMPING;
            body.vy = (body.vy + fy * self.alpha) * DAMPING;
            body.x = clamp_axis(body.x + body.vx, radii[i], VIEW_W);
            body.y = clamp_axis(body.y + body.vy, radii[i], VIEW_H);
        }

        if self.dragged.is_some() {
            self.alpha = self.alpha.max(DRAG_ALPHA);
            true
        } else {
            self.alpha *= ALPHA_DECAY;
            self.alpha > ALPHA_MIN
        }
    }

    fn drag_to(&mut self, i: usize, x: f64, y: f64, radius: f64) {
        if let Some(body) = self.bodies.get_mut(i) {
            body.x = clamp_axis(x, radius, VIEW_W);
            body.y = clamp_axis(y, radius, VIEW_H);
        }
    }
}

fn clamp_axis(v: f64, radius: f64, size: f64) -> f64 {
    v.clamp(radius + 4.0, size - radius - 4.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

// ─── Layout ───────────────────────────────────────────────────────────────────

/// Abstract target distance: 0.3 for perfectly correlated (so circles don't
/// overlap), up to 2.3 for perfectly inverse.
fn target_distance(c: f64) -> f64 {
    0.3 + (1.0 - c)
}

/// Spring rest lengths in view-box pixels, scaled so the whole graph fits.
fn spring_lengths(corr: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let scale = VIEW_H.min(VIEW_W) * 0.18;
    corr.iter()
        .map(|row| row.iter().map(|c| target_distance(*c) * scale).collect())
        .collect()
}

/// Places nodes so their distances approximate [`target_distance`]
/// (stress majorization), then fits them into the view box. Used as the
/// simulation's starting point so it begins near equilibrium.
fn layout(corr: &[Vec<f64>]) -> Vec<(f64, f64)> {
    let n = corr.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![(VIEW_W / 2.0, VIEW_H / 2.0)];
    }

    // Deterministic start on a circle.
    let mut pos: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            (a.cos(), a.sin())
        })
        .collect();

    for _ in 0..300 {
        for i in 0..n {
            let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
            for j in (0..n).filter(|&j| j != i) {
                let d = target_distance(corr[i][j]);
                let w = 1.0 / (d * d);
                let (dx, dy) = (pos[i].0 - pos[j].0, pos[i].1 - pos[j].1);
                let dist = dx.hypot(dy).max(1e-6);
                sx += w * (pos[j].0 + d * dx / dist);
                sy += w * (pos[j].1 + d * dy / dist);
                sw += w;
            }
            pos[i] = (sx / sw, sy / sw);
        }
    }

    fit_to_view(&pos)
}

/// Uniformly scales and centres points into the padded view box.
fn fit_to_view(pos: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (min_x, max_x) = min_max(pos.iter().map(|p| p.0));
    let (min_y, max_y) = min_max(pos.iter().map(|p| p.1));
    let span_x = (max_x - min_x).max(1e-6);
    let span_y = (max_y - min_y).max(1e-6);
    let scale = ((VIEW_W - 2.0 * PADDING) / span_x).min((VIEW_H - 2.0 * PADDING) / span_y);
    let off_x = (VIEW_W - span_x * scale) / 2.0;
    let off_y = (VIEW_H - span_y * scale) / 2.0;
    pos.iter()
        .map(|(x, y)| ((x - min_x) * scale + off_x, (y - min_y) * scale + off_y))
        .collect()
}

fn min_max(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
        (a.0 - b.0).hypot(a.1 - b.1)
    }

    // 0 and 1 move together, 2 moves opposite to both.
    fn corr() -> Vec<Vec<f64>> {
        vec![
            vec![1.0, 0.9, -0.5],
            vec![0.9, 1.0, -0.4],
            vec![-0.5, -0.4, 1.0],
        ]
    }

    #[test]
    fn layout_puts_related_stocks_closer() {
        let pos = layout(&corr());
        assert!(dist(pos[0], pos[1]) < dist(pos[0], pos[2]));
        assert!(dist(pos[0], pos[1]) < dist(pos[1], pos[2]));
    }

    #[test]
    fn simulation_settles_with_related_stocks_closer() {
        let corr = corr();
        let radii = vec![16.0; 3];
        let mut sim = Sim {
            bodies: vec![
                Body::at(100.0, 100.0),
                Body::at(500.0, 300.0),
                Body::at(300.0, 190.0),
            ],
            alpha: 1.0,
            dragged: None,
        };
        let mut frames = 0;
        while sim.step(&spring_lengths(&corr), &radii) {
            frames += 1;
            assert!(frames < 1000, "simulation never settled");
        }
        let p: Vec<_> = sim.bodies.iter().map(Body::pos).collect();
        assert!(dist(p[0], p[1]) < dist(p[0], p[2]));
        assert!(dist(p[0], p[1]) < dist(p[1], p[2]));
    }

    #[test]
    fn dragged_node_stays_pinned() {
        let mut sim = Sim {
            bodies: vec![Body::at(100.0, 100.0), Body::at(500.0, 300.0)],
            alpha: 1.0,
            dragged: Some(0),
        };
        let targets = spring_lengths(&[vec![1.0, 0.0], vec![0.0, 1.0]]);
        for _ in 0..50 {
            assert!(sim.step(&targets, &[16.0, 16.0]));
        }
        assert_eq!(sim.bodies[0].pos(), (100.0, 100.0));
    }
}
