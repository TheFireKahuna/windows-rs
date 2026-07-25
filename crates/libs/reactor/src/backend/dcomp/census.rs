//! Exact counts of what this window has put in front of the compositor.
//!
//! The composition engine renders one frame per committed batch, and the work in
//! that frame is dominated by walking the visual tree rather than by the pixels
//! that changed. So the first question about any composition performance problem
//! is "how many visuals are there, and who owns them" — and until this module
//! existed the answer was a guess. The crate-level
//! [`Census`](windows_composition::Census) counts the traffic through the
//! wrapper layer; this counts the standing population, three ways that check
//! each other:
//!
//! * **The walk.** A depth-first enumeration of the live compositor tree from
//!   the root down. This is ground truth: it asks the compositor what is
//!   actually parented rather than trusting any bookkeeping of ours, so a visual
//!   we forgot about still shows up in it.
//! * **The model.** Per node, the visuals that node owns directly — its chrome
//!   parts, glyph sprites, bars, trace layers — obtained by counting its
//!   container's direct children and subtracting the child nodes among them.
//!   Summed by [`ControlKind`], this is what attributes the walk's total.
//! * **The running tally.** `inserts - removes` from the wrapper layer.
//!
//! Where the three disagree is itself the finding. A walk larger than the model
//! means visuals are parented that no node claims; a tally larger than the walk
//! means something was inserted into a subtree that is no longer attached.
//!
//! Everything here is on demand and O(tree). Nothing runs unless a report is
//! asked for.

use core::fmt::Write as _;

use windows_composition::{OverdrawKinds, Visual};

use super::bootstrap::Compositing;
use super::node::Arena;
use crate::backend::{ControlId, ControlKind};

/// Which heat map the compositor should paint over the window.
///
/// Redraw is the one that answers the question this module exists for: it shows
/// the region the compositor actually recomposed, which is derived from the
/// batches the app committed and is therefore the only honest measure of how
/// much of the window our updates cost. A window that tints edge to edge every
/// frame is asking for a full recomposite whatever its own counters say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeatMap {
    /// Regions recomposed, per frame.
    Redraw,
    /// Pixels more than one visual contributes to, for the selected content.
    Overdraw(OverdrawKinds),
    /// GPU memory held by the content.
    MemoryUsage,
}

/// What a depth-first walk of the live compositor tree found.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TreeCensus {
    /// Every visual below the root, the root itself excluded.
    pub visuals: usize,
    /// Visuals with at least one child.
    pub branches: usize,
    /// Visuals with none — the population that produces pixels and nothing else.
    pub leaves: usize,
    /// Deepest path from the root, in edges.
    pub max_depth: usize,
    /// The largest single child collection encountered.
    pub widest: usize,
    /// Visuals that are in the tree but draw nothing — themselves invisible or
    /// fully transparent, or somewhere beneath one that is.
    ///
    /// This is the whole of the pruning opportunity, and the reason to measure
    /// it rather than assume it. Detaching a subtree, or clearing `IsVisible` on
    /// its root, can only save the compositor work it is currently doing; a
    /// visual already outside the tree costs nothing to walk and a hidden
    /// subtree of one is not worth restructuring for. If this is near zero there
    /// is nothing here to win, whatever the total says.
    pub under_hidden: usize,
    /// Of those, the ones whose own root is the hidden visual — the subtrees a
    /// single property write could take out.
    pub hidden_roots: usize,
    /// How those roots hide, which decides what can be done about them. A
    /// visual held at zero opacity is the platform's documented anti-pattern and
    /// wants removing from the tree; one already invisible has taken the only
    /// step the API offers and still costs a walk, so the lever there is not to
    /// have the visual at all.
    pub hidden_by_visibility: usize,
    pub hidden_by_opacity: usize,
    /// Collections that refused enumeration. Non-zero means the walk is a lower
    /// bound rather than a count, and the rest of the numbers must be read that
    /// way.
    pub unwalkable: usize,
}

impl TreeCensus {
    /// Walk `root`'s subtree. The root is not counted — it is the frame of
    /// reference, not content.
    pub(crate) fn walk(root: &Visual) -> Self {
        let mut out = Self::default();
        descend(root, 0, false, &mut out);
        out
    }
}

fn descend(visual: &Visual, depth: usize, hidden: bool, out: &mut TreeCensus) {
    let Some(container) = visual.as_container() else {
        out.leaves += 1;
        return;
    };
    let Ok(children) = container.children().iter() else {
        out.unwalkable += 1;
        out.leaves += 1;
        return;
    };
    let mut count = 0usize;
    for child in children {
        count += 1;
        out.visuals += 1;
        out.max_depth = out.max_depth.max(depth + 1);
        // A hidden visual's descendants are hidden too, so the flag only ever
        // turns on: it is the subtree that is prunable, not the one node.
        // Composition properties are write-only in principle — a getter can be
        // stale the moment it returns, and reading one is not free — but for a
        // value the app itself last wrote, on an on-demand diagnostic walk, both
        // caveats are affordable.
        let invisible = !child.is_visible();
        let transparent = child.opacity() <= 0.0;
        let child_hidden = hidden || invisible || transparent;
        if child_hidden {
            out.under_hidden += 1;
            if !hidden {
                out.hidden_roots += 1;
                // Visibility first: a visual that is both is already pruned as
                // far as the API allows, and counting it as an opacity hide
                // would overstate what removing it from the tree could win.
                if invisible {
                    out.hidden_by_visibility += 1;
                } else {
                    out.hidden_by_opacity += 1;
                }
            }
        }
        descend(&child, depth + 1, child_hidden, out);
    }
    if count == 0 {
        out.leaves += 1;
    } else {
        out.branches += 1;
        out.widest = out.widest.max(count);
    }
}

/// One row of the per-kind attribution.
#[derive(Clone, Copy)]
struct KindRow {
    kind: ControlKind,
    nodes: usize,
    /// Visuals these nodes own directly — every child of their containers that
    /// is not another node's container.
    own_visuals: usize,
}

/// The reactor's own view: how many nodes exist, of what kind, and how much
/// compositor content each kind carries.
pub(crate) struct ModelCensus {
    rows: Vec<KindRow>,
    pub nodes: usize,
    pub own_visuals: usize,
    pub with_parts: usize,
    pub with_bars: usize,
    pub with_trace: usize,
    /// Nodes whose container would not enumerate; their content is missing from
    /// `own_visuals`.
    pub unwalkable: usize,
}

impl ModelCensus {
    pub(crate) fn take(arena: &Arena) -> Self {
        let mut out = Self {
            rows: Vec::new(),
            nodes: 0,
            own_visuals: 0,
            with_parts: 0,
            with_bars: 0,
            with_trace: 0,
            unwalkable: 0,
        };
        for (_id, node) in arena.iter() {
            out.nodes += 1;
            out.with_parts += usize::from(node.parts.is_some());
            out.with_bars += usize::from(node.bars.is_some());
            out.with_trace += usize::from(node.trace.is_some());

            // A node's container holds both its own chrome and the containers of
            // its child nodes. Only the former is this node's content, and the
            // child count is exactly how many of the latter there are — so the
            // subtraction attributes every visual to the node that made it,
            // without double-counting a subtree at every level above it.
            let direct = match node.container.children().iter() {
                Ok(children) => children.count(),
                Err(_) => {
                    out.unwalkable += 1;
                    continue;
                }
            };
            let own = direct.saturating_sub(node.children.len());
            out.own_visuals += own;

            match out.rows.iter_mut().find(|r| r.kind == node.kind) {
                Some(row) => {
                    row.nodes += 1;
                    row.own_visuals += own;
                }
                None => out.rows.push(KindRow {
                    kind: node.kind,
                    nodes: 1,
                    own_visuals: own,
                }),
            }
        }
        // Heaviest first: the question this answers is always "what is producing
        // the visuals", and an alphabetical list buries that.
        out.rows.sort_by(|a, b| b.own_visuals.cmp(&a.own_visuals));
        out
    }
}

/// Assemble the full report: the crate-level traffic counters, the authoritative
/// walk, and the per-kind attribution, with the three totals side by side so a
/// disagreement is visible rather than inferred.
///
/// Takes the backend whole rather than its pieces: the walk and the model census
/// must describe the *same* instant, and handing over three borrows invites a
/// caller to pair a tree with someone else's arena.
pub(crate) fn report(backend: &super::DCompBackend) -> String {
    render(&backend.comp, &backend.arena, backend.attached_root)
}

fn render(comp: &Compositing, arena: &Arena, attached: Option<ControlId>) -> String {
    let traffic = windows_composition::census();
    let tree = TreeCensus::walk(comp.root_visual());
    let model = ModelCensus::take(arena);

    let mut s = String::with_capacity(2048);
    let _ = writeln!(s, "── composition census ──────────────────────────────");
    let _ = writeln!(
        s,
        "  tree (walked)    visuals {:>7}  branches {:>6}  leaves {:>7}",
        tree.visuals, tree.branches, tree.leaves,
    );
    let _ = writeln!(
        s,
        "                   depth   {:>7}  widest   {:>6}  unwalkable {:>3}",
        tree.max_depth, tree.widest, tree.unwalkable,
    );
    let _ = writeln!(
        s,
        "                   hidden  {:>7}  roots    {:>6}  drawn  {:>7}",
        tree.under_hidden,
        tree.hidden_roots,
        tree.visuals.saturating_sub(tree.under_hidden),
    );
    let _ = writeln!(
        s,
        "                   by IsVisible {:>4}  by opacity {:>4}",
        tree.hidden_by_visibility, tree.hidden_by_opacity,
    );
    let _ = writeln!(
        s,
        "  model            nodes   {:>7}  own vis  {:>6}  unwalkable {:>3}",
        model.nodes, model.own_visuals, model.unwalkable,
    );
    let _ = writeln!(
        s,
        "                   parts   {:>7}  bars     {:>6}  traces {:>7}",
        model.with_parts, model.with_bars, model.with_trace,
    );
    let _ = writeln!(
        s,
        "  attached root    {}",
        match attached {
            Some(id) => format!("node {}", id.get()),
            None => "none — the tree below is chrome only".into(),
        },
    );
    let _ = writeln!(s, "{traffic}");
    {
        // The trace geometry gate: how many publishes restated a shape already on
        // screen, and so cost a comparison instead of a walk, a widen and a new
        // composition path.
        let (applied, skipped) = super::live_trace::geom_gate();
        let total = applied + skipped;
        let _ = writeln!(
            s,
            "  trace geometry   applied {:>7}  skipped {:>6}  gated {:>5.1}%",
            applied,
            skipped,
            if total == 0 { 0.0 } else { skipped as f64 * 100.0 / total as f64 },
        );
    }
    let _ = writeln!(s, "  by kind (own visuals, heaviest first)");
    for row in model.rows.iter().take(20) {
        let _ = writeln!(
            s,
            "    {:<22} nodes {:>5}  visuals {:>6}",
            format!("{:?}", row.kind),
            row.nodes,
            row.own_visuals,
        );
    }
    if model.rows.len() > 20 {
        let tail: usize = model.rows[20..].iter().map(|r| r.own_visuals).sum();
        let _ = writeln!(
            s,
            "    {:<22} kinds {:>5}  visuals {:>6}",
            format!("(+{} more kinds)", model.rows.len() - 20),
            model.rows.len() - 20,
            tail,
        );
    }
    s
}

// ── Reaching the report from outside the front thread ────────────────────────

/// Print a census to stderr. Must run on the front thread — the visual tree is
/// thread-affine, and a walk from anywhere else is undefined rather than merely
/// wrong. Off-thread callers go through [`request`].
pub(crate) fn dump() {
    let Some(shared) = super::host::shared() else {
        eprintln!("reactor census: no host on this thread");
        return;
    };
    let text = report(&shared.backend.borrow());
    eprint!("{text}");
}

/// Ask the front thread for a census from wherever the caller happens to be.
/// Silently does nothing before the window exists, which is the ordinary case
/// for a periodic sampler that starts with the process.
pub fn request() {
    let hwnd = super::live_text::front_hwnd();
    if hwnd != 0 {
        super::host::post_ui(hwnd, dump);
    }
}

/// Set (or clear) the compositor heat map over the whole window, from any
/// thread. Reports nothing back: the answer is on the screen.
pub fn request_heat_map(map: Option<HeatMap>) {
    let hwnd = super::live_text::front_hwnd();
    if hwnd == 0 {
        return;
    }
    super::host::post_ui(hwnd, move || {
        let Some(shared) = super::host::shared() else {
            return;
        };
        match shared.backend.borrow().comp.set_heat_map(map) {
            Ok(true) => {}
            // Both failures are worth a line, and the first one names its cause:
            // an investigation that quietly shows no heat map reads as "nothing
            // is being redrawn", which is the opposite of the truth. The
            // compositor withholds debug settings entirely unless the machine is
            // in developer mode, and that is a machine setting rather than
            // anything the app can arrange for itself.
            Ok(false) => eprintln!(
                "reactor census: the compositor withheld debug settings — heat maps require \
                 Windows developer mode (Settings ▸ System ▸ Advanced ▸ For developers), \
                 which is off on this machine"
            ),
            Err(e) => eprintln!("reactor census: heat map unavailable ({e})"),
        }
    });
}

/// Apply whatever the environment asks for at host startup: a periodic census
/// (`REACTOR_CENSUS=<seconds>`) and a heat map (`REACTOR_HEATMAP=<name>`).
///
/// Driven by the environment rather than by app code so that a capture harness
/// — which launches a prebuilt binary and cannot call into it — can still turn
/// both on. Nothing here runs unless a variable is set.
pub(crate) fn start_from_env() {
    start_periodic();
    start_heat_map();
}

/// `REACTOR_HEATMAP=redraw | overdraw | offscreen | memory | off`.
///
/// `offscreen` is `overdraw` narrowed to content the compositor had to render to
/// an intermediate surface first — the cost a visual-surface mask or an effect
/// brush imposes, which no other view separates out.
fn start_heat_map() {
    let Ok(spec) = std::env::var("REACTOR_HEATMAP") else {
        return;
    };
    let map = match spec.trim().to_ascii_lowercase().as_str() {
        "off" | "none" | "" => None,
        "redraw" => Some(HeatMap::Redraw),
        "overdraw" => Some(HeatMap::Overdraw(OverdrawKinds::All)),
        "offscreen" => Some(HeatMap::Overdraw(OverdrawKinds::OffscreenRendered)),
        "memory" => Some(HeatMap::MemoryUsage),
        other => {
            eprintln!(
                "reactor census: REACTOR_HEATMAP={other:?} — expected redraw, overdraw, offscreen, memory or off"
            );
            return;
        }
    };
    request_heat_map(map);
}

/// Start the periodic sampler if `REACTOR_CENSUS` names an interval in seconds.
///
/// A sampler rather than a one-shot because the numbers that matter are
/// standing populations under load: a census taken at startup describes a window
/// that has not yet drawn anything. Driven by its own thread rather than by the
/// frame pump, so an idle window — the case where the population should be
/// *stable* and the traffic zero — is still reported.
fn start_periodic() {
    let Ok(spec) = std::env::var("REACTOR_CENSUS") else {
        return;
    };
    let Ok(secs) = spec.trim().parse::<f32>() else {
        eprintln!("reactor census: REACTOR_CENSUS={spec:?} is not a number of seconds");
        return;
    };
    if !(secs.is_finite() && secs > 0.0) {
        return;
    }
    let period = std::time::Duration::from_secs_f32(secs);
    // Detached: the process exiting is the only thing that stops it, and a
    // diagnostic sampler has nothing to clean up.
    let _ = std::thread::Builder::new()
        .name("reactor-census".into())
        .spawn(move || loop {
            std::thread::sleep(period);
            request();
        });
}
