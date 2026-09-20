//! Parent-first controls: their children register retained parts with the live owner.

use super::binding::Retired;
use super::host::Host;
use super::text::MeasureKey;
use super::tree;
use super::ui::{Element, Ui};
use crate::gesture::{DragDecl, DragUpdate, GestureDecl};
use crate::layout::{Align, Len, Preset};
use crate::overlay::{Request, Side, Spec};
use crate::role::{Metric, Role, Scope, Text};
use crate::signal::{Cell, Signal};
use crate::text_input::InputScope;
use crate::widget::{
    Chrome, ChromeRow, Gesturing, Interaction, ModelState, Range, ScalarPart, ScalarValue,
    TextSource, TextStyle, UiaRole, ValueRow, Wash, flag, roles,
};
use std::borrow::Cow;
use std::rc::Rc;
use windows_scene::{ControlId, HitDecl, HitFlags, NodeId, Prop, Value};

/// A control whose callbacks exchange scalar values, rather than field text.
pub struct Scalar;

/// One control's application-side callbacks.
///
/// One handler per gesture and not one per phase: a gesture is a sequence with exactly one
/// end, and a handler per phase would let a caller register the moves and forget the release.
///
/// Every field is an `Rc`, because running one is application code and must not hold the
/// host's borrow: the dispatcher clones the handler out before it calls it, and the overlay
/// layer clones a description or a flyout body out before it builds one.
#[derive(Default)]
pub(crate) struct Handlers {
    pub click: Option<Rc<dyn Fn()>>,
    pub scalar: Option<Rc<dyn Fn(Gesturing<f64>)>>,
    pub drag: Option<Rc<dyn Fn(Gesturing<DragUpdate>)>>,
    pub commit: Option<Rc<dyn Fn(&str)>>,
    /// The hover description and the side it opens on.
    ///
    /// The side is authored rather than derived: which side clears a control's neighbours
    /// depends on the axis its author stacked them on, so a description below a toolbar
    /// button clears its neighbours and the same one below a rail item lands on the next.
    pub tip: Option<(Rc<TextSource>, Side)>,
    pub flyout: Option<Rc<dyn Fn(&mut Ui<'_>)>>,
}

/// One interactive control. Its index is its slot for the life of its mount.
pub(crate) struct ControlRow {
    /// The row the scene thread reads: wash sprite, resolved alphas, scope and flags.
    pub front: ChromeRow,
    /// The value half, placed only where a value moves.
    pub value: Option<ValueRow>,
    /// This control's row in the handler table, or [`tree::NONE`] where it declared none.
    pub handlers: u32,
    /// Literal names stay borrowed; generated names are released with the control.
    pub name: Option<Cow<'static, str>>,
    pub key: Option<&'static str>,
    /// The run automation derives this control's name from where it was given none.
    pub text: Option<MeasureKey>,
    /// Where a gesture publishes its in-flight value for as long as it owns one.
    pub live: Option<Cell<Option<f64>>>,
    pub hovered: Option<Cell<bool>>,
    pub validation: Option<&'static str>,
    /// The number the application last published, in its own units, or `None` where the
    /// application publishes none and the value is written elsewhere — a presented read-out's
    /// is its renderer's. Automation reports this rather than the fraction beside it, which is
    /// an `f32` and lands a millionth away from the number a client just wrote.
    pub number: Option<f64>,
    /// The overlay this control is the slot root of, where it is one. What decides the
    /// element its contents are announced inside, and which opening and closing event the
    /// publish owes.
    pub overlay: Option<crate::overlay::Kind>,
    /// Whether the control is one of a selection, which is a capability rather than a state:
    /// a row that is not selected still answers `SelectionItem`, or a client enumerating a
    /// list would find only the row already chosen.
    pub selectable: bool,
    pub node: NodeId,
    pub scope: Scope,
    pub state: ModelState,
    pub uia: UiaRole,
}

impl ControlRow {
    /// A control with no chrome and nothing to light: what a blocker and a scroll rail take.
    pub fn blank(node: NodeId, scope: Scope) -> Self {
        Self {
            front: ChromeRow::default(),
            value: None,
            handlers: tree::NONE,
            name: None,
            key: None,
            text: None,
            live: None,
            hovered: None,
            validation: None,
            number: None,
            overlay: None,
            selectable: false,
            node,
            scope,
            state: ModelState::Rest,
            uia: UiaRole::None,
        }
    }
}

// -- construction ---------------------------------------------------------------------

impl Ui<'_> {
    /// The control exists before its body; nested controls start a new part-ownership scope.
    pub fn control(
        &mut self,
        chrome: Option<Chrome>,
        role: UiaRole,
        children: impl FnOnce(&mut Ui<'_>),
    ) -> Element<'_> {
        self.control_as(chrome, role, children)
    }

    pub fn button(
        &mut self,
        chrome: Chrome,
        typography: TextStyle,
        text: impl Into<TextSource>,
    ) -> Element<'_> {
        let text = text.into();
        self.control(Some(chrome), UiaRole::Button, |ui| {
            if !text.is_empty() {
                ui.text(typography, text);
            }
        })
        .wash(Wash::Ink)
    }

    pub fn field(
        &mut self,
        chrome: Chrome,
        style: TextStyle,
        source: impl Into<TextSource>,
    ) -> Element<'_, super::Field> {
        let element = self.control_as::<super::Field>(Some(chrome), UiaRole::Edit, |ui| {
            // Out of flow: the run is absolute inside the field, so a long edit neither widens
            // the field nor moves its neighbours, and reveal moves the run.
            ui.text(style, "")
                .anchor(0.0, 0.0, [Align::Start, Align::Center]);
        });
        let mut element = element.hit(HitFlags::TEXT, UiaRole::Edit);
        let id = element.control_id();
        element.host().install_field(id, source.into());
        element
    }

    pub fn scalar<M>(
        &mut self,
        chrome: Option<Chrome>,
        drive: Interaction,
        value: impl Signal<ScalarValue, M> + 'static,
        children: impl FnOnce(&mut Ui<'_>),
    ) -> Element<'_, Scalar> {
        self.control_as::<Scalar>(chrome, UiaRole::Slider, children)
            .drive(drive, value)
    }

    /// A two-state control: a track that fills when it is on, and a knob over its travel.
    ///
    /// The on-state fill is the chrome ladder's own selected row rather than a second plate
    /// faded over the first, so the state change is the crossfade every other control's is.
    pub fn toggle<M>(&mut self, on: impl Signal<bool, M> + Copy + 'static) -> Element<'_> {
        let chrome = Chrome::new(roles::TRACK[roles::TRACK_OFF as usize], Metric::RadiusPill)
            .when(ModelState::Selected, roles::TRACK[roles::TRACK_ON as usize]);
        self.control(Some(chrome), UiaRole::CheckBox, |ui| {
            ui.plate(Metric::RadiusPill, Role::Text(Text::Primary), 1.0)
                .size(Len::times(Metric::TrackH, 0.8))
                .scalar_part(ScalarPart::Thumb { vertical: false });
        })
        .selected(on)
        .driven(Interaction::Press, Flag(on))
        .height(Metric::TrackH)
        .width(Len::times(Metric::TrackH, 1.7))
        .padding(Len::times(Metric::TrackH, 0.1))
        .justify(Align::Start)
        .align(Align::Center)
    }

    /// Mints the control row, runs `children` inside its part-ownership scope, and attaches
    /// the chrome its recipe named.
    fn control_as<K>(
        &mut self,
        chrome: Option<Chrome>,
        role: UiaRole,
        children: impl FnOnce(&mut Ui<'_>),
    ) -> Element<'_, K> {
        let node = self.node(Preset::Row).node_id();
        let scope = self.scope();
        let inherited = self
            .host
            .control(self.control)
            .map_or(ControlId::NONE, |row| row.front.scope);
        let id = self.host.mint_control(ControlRow::blank(node, scope));
        self.host.tree.c.control[node.index()] = id;
        if let Some(row) = self.host.control_mut(id) {
            row.uia = role;
            // The enclosing control's scope reaches this one, so a reveal declared above a
            // subtree still names the scope every control under it belongs to.
            row.front.scope = inherited;
        }
        if let Some(chrome) = chrome {
            self.host.declare_chrome(node, chrome);
        }
        let mut element = self.element::<K>(node);
        element.ui.control = id;
        element
            .children(children)
            .hit(HitFlags::INTERACTIVE | HitFlags::GESTURE, role)
    }
}

// -- declaration ----------------------------------------------------------------------

impl<K> Element<'_, K> {
    /// Returns this element's control, minting one at the node's own scope where the element
    /// has not been declared a control yet.
    pub(crate) fn control_id(&mut self) -> ControlId {
        let node = self.node_id();
        let held = self.host().control_of(node);
        if !held.is_none() {
            self.ui.control = held;
            return held;
        }
        let inherited = self.ui.control;
        let scope = self.host().scope_of(node);
        let id = self.host().mint_control(ControlRow::blank(node, scope));
        self.host().tree.c.control[node.index()] = id;
        let inherited = self
            .host()
            .control(inherited)
            .map_or(ControlId::NONE, |row| row.front.scope);
        if let Some(row) = self.host().control_mut(id) {
            row.front.scope = inherited;
        }
        // This element's own context, so children declared after the setter that minted the
        // control attach to it: a part or a reveal below names the control above it.
        self.ui.control = id;
        id
    }

    /// Widens this control's hit flags and names its automation role.
    ///
    /// Additive and idempotent: a control is declared by whichever setter is written first,
    /// and each later one adds what it needs rather than replacing what came before. The hit
    /// array is rebuilt on layout change only, so the flags are recorded here and read there.
    pub(crate) fn hit(mut self, flags: HitFlags, role: UiaRole) -> Self {
        let id = self.control_id();
        let node = self.node_id();
        let held = HitFlags::from_bits(tree::unpack_decl(self.host().tree.c.flags[node.index()]));
        let named = match role {
            UiaRole::None => HitFlags::NONE,
            _ => HitFlags::UIA,
        };
        let inflate = self.host().tree.c.inflate[node.index()];
        self.host().hit(
            node,
            Some(HitDecl {
                flags: held | flags | named,
                id,
                touch_inflate: inflate.is_finite().then_some(inflate),
            }),
        );
        if role != UiaRole::None
            && let Some(row) = self.host().control_mut(id)
        {
            row.uia = role;
        }
        self
    }

    /// Declares this element a control and edits its row.
    fn declare(mut self, flags: HitFlags, write: impl FnOnce(&mut ControlRow)) -> Self {
        let id = self.control_id();
        if let Some(row) = self.host().control_mut(id) {
            write(row);
        }
        self.hit(flags, UiaRole::None)
    }

    /// Declares this element a control and installs one of its handlers.
    ///
    /// The displaced handler is retired rather than dropped here: dropping it runs whatever
    /// the application captured, and that must not happen under the host's borrow.
    fn handler(
        mut self,
        flags: HitFlags,
        write: impl FnOnce(&mut Handlers) -> Option<Retired>,
    ) -> Self {
        let id = self.control_id();
        self.host().set_handler(id, write);
        self.hit(flags, UiaRole::None)
    }

    /// Installs a writer for one destination, or writes the record once where the source is a
    /// constant.
    ///
    /// The fork lives here and not at each setter: a constant installs no effect, and a
    /// reactive source installs one writer belonging to the signal scope creation installed,
    /// so it retires with the subtree that declared it.
    fn bind<T: 'static, M>(
        mut self,
        source: impl Signal<T, M> + 'static,
        write: impl Fn(&mut Host, T) + 'static,
    ) -> Self {
        if source.is_constant() {
            let value = source.read();
            write(self.host(), value);
            return self;
        }
        self.host().binding(move || {
            let value = source.read();
            Host::with(|host| write(host, value));
        });
        self
    }

    pub fn on_click(self, callback: impl Fn() + 'static) -> Self {
        self.handler(HitFlags::INTERACTIVE | HitFlags::GESTURE, |row| {
            row.click.replace(Rc::new(callback)).map(Retired::new)
        })
    }

    /// Literal names stay borrowed; generated names are released with the control.
    pub fn name(self, name: impl Into<Cow<'static, str>>) -> Self {
        let named = self.declare(HitFlags::UIA, |row| row.name = Some(name.into()));
        named.uia_restale()
    }

    fn uia_restale(mut self) -> Self {
        self.host().uia_stale.set(true);
        self
    }

    pub fn key(self, key: &'static str) -> Self {
        self.declare(HitFlags::NONE, |row| row.key = Some(key))
    }

    /// States what this element is to automation, where its widget does not already say.
    ///
    /// A presented read-out is a progress bar and an analyzer is a graph; neither is a
    /// control the widget set minted, so neither carries a role of its own.
    pub fn role(self, role: UiaRole) -> Self {
        self.hit(HitFlags::UIA, role)
    }

    pub fn wash(mut self, wash: Wash) -> Self {
        let node = self.node_id();
        self.host().surface_wash(windows_scene::GroupId(node), wash);
        self.hit(HitFlags::INTERACTIVE, UiaRole::None)
    }

    /// Touch inflation never lets two targets claim one point, so a control sitting inside
    /// another's inflated box states that its own box is the whole of it.
    pub fn no_inflate(self) -> Self {
        self.hit(HitFlags::NO_INFLATE, UiaRole::None)
    }

    pub fn caption(mut self, button: windows_window::CaptionButton) -> Self {
        let id = self.control_id();
        self.host().caption[button as usize] = Some(id);
        self.hit(HitFlags::INTERACTIVE, UiaRole::None)
    }

    pub fn hover_scope(self, hovered: Cell<bool>) -> Self {
        self.interaction_scope()
            .declare(HitFlags::INTERACTIVE, |row| {
                row.hovered = Some(hovered);
                row.front.flags |= flag::OBSERVES;
            })
    }

    /// Groups hover, press and keyboard focus for one retained reveal target.
    /// Declare the scope before mounting its children.
    pub fn interaction_scope(mut self) -> Self {
        let id = self.control_id();
        if let Some(row) = self.host().control_mut(id) {
            row.front.scope = id;
        }
        self.hit(HitFlags::GESTURE, UiaRole::None)
    }

    /// Reveals this element while its enclosing interaction scope is active.
    ///
    /// Each scope accepts one target, which must be mounted below the scope. The front thread
    /// owns its opacity; layout and hit testing remain active.
    ///
    /// # Panics
    ///
    /// Where no interaction scope encloses this element, where the scope already reveals a
    /// different target, or where the application has claimed this node's opacity.
    pub fn reveal_on_interaction(mut self) -> Self {
        let node = self.node_id();
        let enclosing = self.ui.control;
        let scope = self
            .host()
            .control(enclosing)
            .map_or(ControlId::NONE, |row| row.front.scope);
        assert!(!scope.is_none(), "a reveal requires an interaction scope");
        let held = self.host().control(scope).map(|row| row.front.reveal);
        assert!(
            held == Some(NodeId::NONE) || held == Some(node),
            "one reveal target per scope"
        );
        if held == Some(node) {
            return self;
        }
        // The reveal owns opacity, so an application binding on that target is rejected
        // rather than silently overwritten by the front thread.
        assert!(
            self.host().tree.c.channels[node.index()] & (1 << Prop::Opacity as u32) == 0,
            "an interaction reveal requires unclaimed opacity"
        );
        if let Some(row) = self.host().control_mut(scope) {
            row.front.reveal = node;
        }
        self.host()
            .write_channel(node, Prop::Opacity, Value::Scalar(0.0));
        self
    }

    pub fn on_unhandled_escape(mut self, callback: impl Fn() + 'static) -> Self {
        let node = self.node_id();
        self.host().set_escape(node, Rc::new(callback));
        self
    }

    pub fn on_drag(
        self,
        decl: DragDecl,
        callback: impl Fn(Gesturing<DragUpdate>) + 'static,
    ) -> Self {
        let mut dragged = self.handler(HitFlags::GESTURE, |row| {
            row.drag.replace(Rc::new(callback)).map(Retired::new)
        });
        let id = dragged.control_id();
        if let Some(row) = dragged.host().control_mut(id) {
            row.front.flags |= flag::DRAGS;
        }
        dragged
            .host()
            .gestures
            .push((id, GestureDecl::default().with_drag(decl)));
        dragged
    }

    pub fn tip(self, text: impl Into<TextSource>) -> Self {
        self.tip_at(Side::Bottom, text)
    }

    pub fn tip_at(self, side: Side, text: impl Into<TextSource>) -> Self {
        self.handler(HitFlags::INTERACTIVE, |row| {
            row.tip
                .replace((Rc::new(text.into()), side))
                .map(Retired::new)
        })
    }

    pub fn flyout(self, body: impl Fn(&mut Ui<'_>) + 'static) -> Self {
        self.handler(HitFlags::GESTURE | HitFlags::INTERACTIVE, |row| {
            row.flyout.replace(Rc::new(body)).map(Retired::new)
        })
    }

    pub fn popup_when<M>(
        self,
        shown: impl Signal<bool, M> + 'static,
        spec: Spec,
        closed: impl Fn() + 'static,
        body: impl Fn(&mut Ui<'_>) + 'static,
    ) -> Self {
        let node = self.node_id();
        let (body, closed) = (Rc::new(body), Rc::new(closed));
        self.bind(shown, move |host, open| {
            host.popups.push(match open {
                true => Request::Show {
                    key: node,
                    spec: spec.clone(),
                    body: body.clone(),
                    closed: closed.clone(),
                },
                false => Request::Close(node),
            });
        })
    }

    /// Publishes help text only; it does not parse or own validity.
    pub fn validation(mut self, read: impl Fn() -> Option<&'static str> + 'static) -> Self {
        let id = self.control_id();
        self.hit(HitFlags::UIA, UiaRole::None)
            .bind(read, move |host, text| {
                if let Some(row) = host.control_mut(id) {
                    row.validation = text;
                }
                host.uia_stale.set(true);
            })
    }

    /// Marks this descendant a part the enclosing control's fraction moves.
    ///
    /// At most four parts belong to that control, and a duplicate writer for one property is
    /// rejected: the router and the applier reach the same property through one writer, so a
    /// second declaration would be a second writer for it.
    ///
    /// # Panics
    ///
    /// Where no control encloses this element, where the node already binds a property the
    /// part drives, or where the control already holds four parts.
    pub fn scalar_part(mut self, part: ScalarPart) -> Self {
        let owner = self.ui.control;
        assert!(
            !owner.is_none(),
            "a scalar part requires an enclosing control"
        );
        let node = self.node_id();
        assert_eq!(
            self.host().tree.c.channels[node.index()] & claimed(part),
            0,
            "a scalar part cannot also bind its driven property"
        );
        let Some(row) = self.host().control_mut(owner) else {
            return self;
        };
        let parts = &mut row.value.get_or_insert_with(ValueRow::default).parts;
        let slot = parts
            .iter()
            .position(|(held, _)| *held == node)
            .or_else(|| parts.iter().position(|(_, held)| *held == ScalarPart::None))
            .expect("a scalar supports at most four parts");
        parts[slot] = (node, part);
        self
    }

    /// Records the drive, the range and the gesture it implies, and binds the source.
    ///
    /// Generic over the element's marker so a two-state control declares its drive through the
    /// same path a slider does.
    fn driven<M>(
        mut self,
        drive: Interaction,
        value: impl Signal<ScalarValue, M> + 'static,
    ) -> Self {
        let id = self.control_id();
        let (role, decl) = match drive {
            Interaction::Press => (UiaRole::None, GestureDecl::default()),
            Interaction::Slide(range) => (UiaRole::Slider, GestureDecl::slider(range.vertical)),
            // A marker that the control is turned. The pivot it rotates about is the
            // input thread's, which is the side that holds the contact and the box.
            Interaction::Turn(_) => (
                UiaRole::Slider,
                GestureDecl::knob(),
            ),
        };
        if let Some(row) = self.host().control_mut(id) {
            row.front.flags |= bits_of(drive);
            if let Some(range) = range_of(drive) {
                row.value = Some(ValueRow {
                    min: range.min,
                    span: range.max - range.min,
                    step: range.quantum(),
                    ..row.value.unwrap_or_default()
                });
            }
        }
        self.host().gestures.push((id, decl));
        self.hit(HitFlags::INTERACTIVE | HitFlags::GESTURE, role)
            .bind(value, move |host, source| {
                let fraction = range_of(drive)
                    .map_or(source.value as f32, |range| range.fraction(source.value));
                host.publish_fraction(id, fraction, source.value, source.epoch);
            })
    }
}

// -- model state ----------------------------------------------------------------------

impl<K> Element<'_, K> {
    /// Model state, not interaction: a discrete base-paint swap at event rate rather than a
    /// wash the scene thread fades. A disabled control keeps its automation entry and loses
    /// every other hit flag.
    fn model_state<M>(mut self, value: impl Signal<bool, M> + 'static, state: ModelState) -> Self {
        let id = self.control_id();
        self.bind(value, move |host, on| {
            host.set_state(id, Some(if on { state } else { ModelState::Rest }));
        })
    }

    pub fn disabled<M>(self, value: impl Signal<bool, M> + 'static) -> Self {
        self.model_state(value, ModelState::Disabled)
    }

    pub fn selected<M>(mut self, value: impl Signal<bool, M> + 'static) -> Self {
        let node = self.node_id();
        let id = self.control_id();
        self.host().surface_selectable(windows_scene::GroupId(node));
        if let Some(row) = self.host().control_mut(id) {
            row.selectable = true;
        }
        self.model_state(value, ModelState::Selected)
    }
}

// -- scalars --------------------------------------------------------------------------

impl<'a, K> Element<'a, K> {
    /// Binds this control's authoritative value and the gesture its drive implies, making it
    /// a scalar.
    ///
    /// An accepted echo (the same epoch) preserves an active gesture; a changed epoch is a
    /// replaced document and cancels it, restoring the gesture's start without committing. A
    /// source that is never replaced states epoch zero.
    pub fn drive<M>(
        self,
        drive: Interaction,
        value: impl Signal<ScalarValue, M> + 'static,
    ) -> Element<'a, Scalar> {
        self.driven(drive, value).retype()
    }
}

impl Element<'_, Scalar> {
    /// Publishes this control's value while a gesture moves it, and clears `cell` when the
    /// gesture commits or is canceled.
    ///
    /// The value a pointer, a key or an automation client is moving lives on the front thread
    /// until the gesture ends, so a readout beside the control reads it here.
    pub fn live(self, cell: Cell<Option<f64>>) -> Self {
        self.declare(HitFlags::GESTURE, |row| row.live = Some(cell))
    }

    /// Installs the handler this control's gesture reports to.
    pub fn on_gesture(self, callback: impl Fn(Gesturing<f64>) + 'static) -> Self {
        self.handler(HitFlags::GESTURE, |row| {
            row.scalar.replace(Rc::new(callback)).map(Retired::new)
        })
    }
}

impl Element<'_, super::Field> {
    pub fn on_commit(self, callback: impl Fn(&str) + 'static) -> Self {
        self.handler(HitFlags::TEXT, |row| {
            row.commit.replace(Rc::new(callback)).map(Retired::new)
        })
    }

    /// Numeric scope requests an input mode; parsing, units, bounds, validation and
    /// formatting are application policy.
    ///
    /// A masked scope is also the run's fold, so the plaintext never reaches the shaper, the
    /// coverage or the automation snapshot.
    pub fn scope(mut self, scope: InputScope) -> Self {
        let id = self.control_id();
        self.host().set_field_scope(id, scope);
        self
    }
}

/// The range a drive runs over, or `None` for a two-state control.
const fn range_of(drive: Interaction) -> Option<Range> {
    match drive {
        Interaction::Press => None,
        Interaction::Slide(range) | Interaction::Turn(range) => Some(range),
    }
}

/// The chrome-row bits a drive sets: how a pointer reads the value, and which way it grows.
const fn bits_of(drive: Interaction) -> u8 {
    let (kind, range) = match drive {
        Interaction::Press => return 0,
        Interaction::Slide(range) => (flag::SLIDE, range),
        Interaction::Turn(range) => (flag::TURN, range),
    };
    match range.vertical {
        true => kind | flag::VERTICAL,
        false => kind,
    }
}

/// The channel mask a part drives, which is what a second writer would collide with.
///
/// A trail writes no channel from its mapping — it is driven by a compositor expression onto
/// the thumb's own offset — so its two trim endpoints are named here rather than derived.
fn claimed(part: ScalarPart) -> u32 {
    match part {
        ScalarPart::Trail { .. } => (1 << Prop::TrimStart as u32) | (1 << Prop::TrimEnd as u32),
        part => part
            .channels(0.0, 0.0, 1.0)
            .into_iter()
            .flatten()
            .fold(0, |mask, (prop, _)| mask | 1 << prop as u32),
    }
}

/// Lifts a two-state source into the fraction its control stands at.
struct Flag<S>(S);

impl<S: Signal<bool, M>, M> Signal<ScalarValue, M> for Flag<S> {
    fn read(&self) -> ScalarValue {
        let on = self.0.read();
        // The state is the revision. A repeated revision tells the front thread the
        // publication is geometry-only and its own fraction stands, so a source that never
        // states a new one can never move the part it drives. A flag has two values, and a
        // flip is a new one by definition.
        ScalarValue {
            value: f64::from(on),
            epoch: u64::from(on),
        }
    }

    fn is_constant(&self) -> bool {
        self.0.is_constant()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drive_states_how_a_pointer_reads_the_value_and_which_way_it_grows() {
        let up = Range::new(0.0, 1.0).vertical();
        assert_eq!(bits_of(Interaction::Press), 0);
        assert_eq!(bits_of(Interaction::Slide(Range::UNIT)), flag::SLIDE);
        assert_eq!(bits_of(Interaction::Turn(up)), flag::TURN | flag::VERTICAL);
        assert_eq!(range_of(Interaction::Press), None);
        assert_eq!(range_of(Interaction::Slide(Range::UNIT)), Some(Range::UNIT));
    }

    /// A trail drives its two trim endpoints through a compositor expression, so a second
    /// writer on either is what the part declaration rejects.
    #[test]
    fn a_trail_claims_both_trim_endpoints_and_a_thumb_claims_its_offset() {
        let trail = claimed(ScalarPart::Trail { from: 0.0 });
        assert_eq!(
            trail,
            (1 << Prop::TrimStart as u32) | (1 << Prop::TrimEnd as u32)
        );
        assert_ne!(claimed(ScalarPart::Thumb { vertical: false }), 0);
        assert_eq!(claimed(ScalarPart::None), 0);
    }

    #[test]
    fn a_flag_reads_as_the_fraction_its_control_stands_at() {
        assert_eq!(Flag(true).read(), ScalarValue { value: 1.0, epoch: 1 });
        assert_eq!(Flag(false).read(), ScalarValue { value: 0.0, epoch: 0 });
        // Two states, two revisions: a repeated one says the publication is geometry-only
        // and leaves the front thread's own fraction standing.
        assert_ne!(Flag(true).read().epoch, Flag(false).read().epoch);
        assert!(Flag(true).is_constant());
    }
}
