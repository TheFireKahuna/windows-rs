use crate::arena::{Arena, Forest, Held, NO_LINK, desc};
use crate::{NodeId, Prop};
use std::cell::RefCell;
use std::rc::Rc;
use windows_composition::Animatable;
use windows_composition::{
    Clamping, CompositionScopedBatch, CompositionSurfaceBrush, Compositor, ExpressionAnimation,
    InteractionTracker, ScaleAnimationPolicy, TrackerEvent, Vector2, Vector3, Visual,
};
use windows_core::{EventRevoker, Result};

#[derive(Clone, Copy)]
struct Bounds {
    origin: Vector2,
    size: Vector2,
}
struct Motion {
    node: NodeId,
    mask: u8,
    serial: u64,
}
struct State {
    enabled: bool,
    request: Option<i32>,
    serial: u64,
    running: Vec<Motion>,
    current: Bounds,
    target: Bounds,
    publish: Box<dyn FnMut(Vector2, Vector2, Vector2, bool)>,
}
impl State {
    fn emit(&mut self) {
        if self.enabled {
            (self.publish)(
                self.current.origin,
                self.current.size,
                self.target.size,
                !self.running.is_empty(),
            );
        }
    }
    fn replace(&mut self, node: NodeId, mask: u8) {
        self.running.retain_mut(|motion| {
            if motion.node == node {
                motion.mask &= !mask;
            }
            motion.mask != 0
        });
    }
    fn finish(&mut self, serial: u64) {
        self.running.retain(|motion| motion.serial != serial);
        if self.running.is_empty() {
            self.current = self.target;
        }
    }
}
struct Source {
    node: NodeId,
    visual: Visual,
    mask: u8,
}
pub(crate) struct Observer {
    state: Rc<RefCell<State>>,
    tracker: InteractionTracker,
    position: ExpressionAnimation,
    height: ExpressionAnimation,
    sources: Vec<Source>,
    pending: Vec<(NodeId, u8, Rc<CompositionScopedBatch>, EventRevoker)>,
}
fn axes(prop: Prop) -> u8 {
    match prop {
        Prop::Offset => 3,
        Prop::OffsetX => 1,
        Prop::OffsetY => 2,
        Prop::Size => 12,
        Prop::SizeX => 4,
        Prop::SizeY => 8,
        _ => 0,
    }
}
impl Observer {
    pub(crate) fn new(
        compositor: &Compositor,
        node: NodeId,
        nodes: &Arena,
        brush: Option<&CompositionSurfaceBrush>,
        publish: impl FnMut(Vector2, Vector2, Vector2, bool) + 'static,
    ) -> Result<Self> {
        let mut sources = Vec::new();
        let mut at = node;
        loop {
            let Some(visual) = nodes.visual(at) else {
                return Err(crate::invalid_arg());
            };
            let mut mask = 0;
            for (prop, bit) in [(Prop::OffsetX, 1), (Prop::OffsetY, 2)] {
                if nodes.held(at, desc(prop)) != Held::Bound {
                    mask |= bit;
                }
            }
            sources.push(Source {
                node: at,
                visual: visual.clone(),
                mask,
            });
            let parent = nodes.links(at.index() as u32).parent;
            if parent == NO_LINK {
                break;
            }
            at = nodes.id_at(parent);
        }
        let initial = Self::bounds(&sources, nodes);
        let state = Rc::new(RefCell::new(State {
            enabled: true,
            request: None,
            serial: 0,
            running: Vec::new(),
            current: initial,
            target: initial,
            publish: Box::new(publish),
        }));
        let weak = Rc::downgrade(&state);
        let tracker = compositor.create_interaction_tracker_with_owner(move |event| {
            if let TrackerEvent::ValuesChanged {
                position,
                scale,
                request,
            } = event
            {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                let mut state = state.borrow_mut();
                if !state.enabled
                    || state.request != Some(request.0)
                    || !position.x.is_finite()
                    || !position.y.is_finite()
                    || !position.z.is_finite()
                    || !scale.is_finite()
                {
                    return;
                }
                state.current = Bounds {
                    origin: Vector2::new(position.x, position.y),
                    size: Vector2::new(position.z.max(0.0), (scale - 1.0).max(0.0)),
                };
                state.emit();
            }
        })?;
        tracker.set_position_bounds(Vector3::new(-1e6, -1e6, 0.0), Vector3::new(1e6, 1e6, 1e6));
        tracker.set_scale_bounds(1.0, 1e6);
        let sum = |bit, axis: &str| {
            let parts: Vec<_> = sources
                .iter()
                .enumerate()
                .filter(|(_, s)| s.mask & bit != 0)
                .map(|(i, _)| format!("s{i}.Offset.{axis}"))
                .collect();
            if parts.is_empty() {
                "0".to_owned()
            } else {
                parts.join(" + ")
            }
        };
        let x = sum(1, "X");
        let y = sum(2, "Y");
        let position =
            compositor.create_expression_animation(&format!("Vector3({x}, {y}, s0.Size.X)"));
        let height = compositor.create_expression_animation("s0.Size.Y + 1");
        let compensation = compositor.create_expression_animation(&format!("-Vector2({x}, {y})"));
        for (i, source) in sources.iter().enumerate() {
            let name = format!("s{i}");
            position.set_reference_parameter(&name, &source.visual);
            compensation.set_reference_parameter(&name, &source.visual);
        }
        height.set_reference_parameter("s0", &sources[0].visual);
        if let Some(brush) = brush {
            brush.start_animation("Offset", &compensation);
        }
        state.borrow_mut().emit();
        Ok(Self {
            state,
            tracker,
            position,
            height,
            sources,
            pending: Vec::new(),
        })
    }
    fn bounds(sources: &[Source], nodes: &Arena) -> Bounds {
        let mut origin = Vector2::zero();
        for source in sources {
            if source.mask & 1 != 0 {
                origin.x += nodes.chan(source.node, desc(Prop::OffsetX).chan);
            }
            if source.mask & 2 != 0 {
                origin.y += nodes.chan(source.node, desc(Prop::OffsetY).chan);
            }
        }
        Bounds {
            origin,
            size: nodes.size(sources[0].node),
        }
    }
    pub(crate) fn affected(&self, node: NodeId, prop: Prop) -> bool {
        let mask = axes(prop);
        self.sources.iter().any(|source| {
            source.node == node
                && (mask & source.mask != 0 || (node == self.sources[0].node && mask & 12 != 0))
        })
    }
    pub(crate) fn arm(&mut self) -> Result<()> {
        let mut state = self.state.borrow_mut();
        if state.enabled && state.request.is_none() {
            self.tracker
                .try_update_position_with_animation(&self.position)?;
            state.request = Some(
                self.tracker
                    .try_update_scale_with_animation(&self.height, Vector3::zero())?
                    .0,
            );
        }
        Ok(())
    }
    fn stop_tracker(tracker: &InteractionTracker, state: &mut State) -> Result<()> {
        state.request = None;
        let b = state.current;
        tracker.try_update_position(
            Vector3::new(b.origin.x, b.origin.y, b.size.x),
            Clamping::Disabled,
            ScaleAnimationPolicy::Stop,
        )?;
        Ok(())
    }
    pub(crate) fn active(&mut self, enabled: bool) -> Result<()> {
        {
            let mut state = self.state.borrow_mut();
            if state.enabled == enabled {
                return Ok(());
            }
            state.enabled = enabled;
            if !enabled {
                return Self::stop_tracker(&self.tracker, &mut state);
            }
            if state.running.is_empty() {
                state.current = state.target;
                state.emit();
                return Ok(());
            }
        }
        self.arm()
    }
    pub(crate) fn watch(
        &mut self,
        node: NodeId,
        prop: Prop,
        batch: &Rc<CompositionScopedBatch>,
    ) -> Result<()> {
        let mask = axes(prop);
        let serial = {
            let mut state = self.state.borrow_mut();
            state.replace(node, mask);
            state.serial += 1;
            let serial = state.serial;
            state.running.push(Motion { node, mask, serial });
            serial
        };
        self.pending.retain_mut(|(held, bits, _, _)| {
            if *held == node {
                *bits &= !mask;
            }
            *bits != 0
        });
        let weak = Rc::downgrade(&self.state);
        let tracker = self.tracker.clone();
        let revoker = batch.on_completed(move || {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            state.finish(serial);
            if state.running.is_empty() {
                if let Err(error) = Self::stop_tracker(&tracker, &mut state) {
                    eprintln!("native bounds observer could not stop: {error}");
                }
            }
            state.emit();
        })?;
        self.pending.push((node, mask, batch.clone(), revoker));
        Ok(())
    }
    pub(crate) fn target(&mut self, nodes: &Arena) {
        let mut state = self.state.borrow_mut();
        state.target = Self::bounds(&self.sources, nodes);
        if state.running.is_empty() {
            state.current = state.target;
        }
        state.emit();
    }
    pub(crate) fn direct(&mut self, node: NodeId, prop: Prop, nodes: &Arena) -> Result<()> {
        let mask = axes(prop);
        {
            let mut state = self.state.borrow_mut();
            state.replace(node, mask);
            if state.running.is_empty() {
                Self::stop_tracker(&self.tracker, &mut state)?;
            }
        }
        self.pending.retain_mut(|(held, bits, _, _)| {
            if *held == node {
                *bits &= !mask;
            }
            *bits != 0
        });
        self.target(nodes);
        Ok(())
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.enabled = false;
        let _ = Self::stop_tracker(&self.tracker, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_retargets_and_ancestor_completions_settle_only_current_motion() {
        let a = NodeId::raw(1, 1);
        let b = NodeId::raw(2, 1);
        let initial = Bounds {
            origin: Vector2::zero(),
            size: Vector2::new(100.0, 50.0),
        };
        let mut state = State {
            enabled: true,
            request: None,
            serial: 3,
            running: vec![
                Motion {
                    node: a,
                    mask: 12,
                    serial: 1,
                },
                Motion {
                    node: b,
                    mask: 3,
                    serial: 2,
                },
            ],
            current: initial,
            target: Bounds {
                origin: Vector2::new(20.0, 10.0),
                size: Vector2::new(80.0, 40.0),
            },
            publish: Box::new(|_, _, _, _| {}),
        };
        state.replace(a, 4);
        state.running.push(Motion {
            node: a,
            mask: 4,
            serial: 3,
        });
        state.finish(1);
        assert_eq!(state.running.len(), 2);
        state.finish(2);
        assert_eq!(state.running.len(), 1);
        assert_eq!(state.current.origin, initial.origin);
        state.finish(3);
        assert!(state.running.is_empty());
        assert_eq!(state.current.origin, state.target.origin);
        assert_eq!(state.current.size, state.target.size);
    }
}
