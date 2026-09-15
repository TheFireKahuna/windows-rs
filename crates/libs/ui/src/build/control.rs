//! Parent-first controls: their children register retained parts with the live owner.
use super::binding::{Destination, Retired};
use super::host::ControlRow;
use super::theme::Part;
use super::ui::Target;
use super::{Any, Element, Host, Ui};
use crate::layout::{Align, Len, Preset};
use crate::role::{Elevation, Metric, Role, Scope, Text};
use crate::signal::Signal;
use crate::widget::{Chrome, ModelState, TextSource, TextStyle, UiaRole, Wash};
use std::rc::Rc;
use windows_scene::{ControlId, HitDecl, HitFlags, NodeId};

/// A control whose callbacks exchange scalar values, rather than field text.
#[derive(Copy, Clone, Debug)]
pub struct Scalar;

impl Ui<'_> {
    pub fn toggle<M>(&mut self, on: impl Signal<bool, M> + Copy + 'static) -> Element<'_> {
        let chrome = Chrome::new(
            crate::widget::roles::TRACK[crate::widget::roles::TRACK_OFF as usize],
            Metric::RadiusPill,
        );
        let mut element = self
            .control(Some(chrome), UiaRole::CheckBox, |ui| {
                let fill = ui
                    .plate(
                        Metric::RadiusPill,
                        Role::Fill(
                            crate::widget::roles::TRACK[crate::widget::roles::TRACK_ON as usize]
                                .fill
                                .unwrap(),
                        ),
                        1.0,
                    )
                    .cover();
                if on.is_constant() {
                    fill.opacity(f32::from(on.read()));
                } else {
                    fill.opacity(move || f32::from(on.read()));
                }
                ui.plate(
                    Len::Times(Metric::RowH, 0.4),
                    Role::Text(Text::Primary),
                    1.0,
                )
                .width(Len::Times(Metric::RowH, 0.8))
                .height(Len::Times(Metric::RowH, 0.8))
                .thumb();
            })
            .selected(on)
            .min_height(Len::Zero)
            .height(Metric::TrackH)
            .width(Len::Times(Metric::TrackH, 1.7))
            .padding(Len::Times(Metric::TrackH, 0.1))
            .justify(Align::Start)
            .align(Align::Center)
            .no_shrink();
        let id = element.control_id(HitFlags::INTERACTIVE);
        element.host.controls.get_mut(id).unwrap().front.drive =
            Some(crate::widget::Interaction::Press);
        if on.is_constant() {
            element.host.publish_fraction(id, f32::from(on.read()), 0);
        } else {
            element
                .host
                .bind_to(element.node.target.id(), Destination::Scalar, move || {
                    let fraction = f32::from(on.read());
                    Host::with(|host| host.publish_fraction(id, fraction, 0));
                });
        }
        element
    }
    pub fn scalar<M>(
        &mut self,
        chrome: Option<Chrome>,
        drive: crate::widget::Interaction,
        value: impl Signal<crate::widget::ScalarValue, M> + 'static,
        children: impl FnOnce(&mut Ui<'_>),
    ) -> Element<'_, Scalar> {
        let mut element = self.control_as::<Scalar>(chrome, UiaRole::Slider, children);
        let id = element.control_id(HitFlags::GESTURE);
        let range = match drive {
            crate::widget::Interaction::Slide(range) | crate::widget::Interaction::Turn(range) => {
                range
            }
            crate::widget::Interaction::Press => panic!("a scalar requires a range"),
        };
        element.host.controls.get_mut(id).unwrap().front.drive = Some(drive);
        let gesture = match drive {
            crate::widget::Interaction::Slide(range) => {
                crate::gesture::GestureDecl::slider(range.vertical)
            }
            _ => crate::gesture::GestureDecl {
                drag: Some(crate::gesture::DragDecl::turn()),
                ..Default::default()
            },
        };
        element.host.gestures.push((id, gesture));
        let node = element.node.target.id();
        if value.is_constant() {
            let value = value.read();
            element
                .host
                .publish_fraction(id, range.fraction(value.value), value.epoch);
        } else {
            element.host.bind_to(node, Destination::Scalar, move || {
                let value = value.read();
                Host::with(|host| {
                    host.publish_fraction(id, range.fraction(value.value), value.epoch)
                });
            });
        }
        element
    }

    pub fn field(
        &mut self,
        chrome: Chrome,
        style: TextStyle,
        source: impl Into<TextSource>,
    ) -> Element<'_, super::Field> {
        let mut element = self.control_as::<super::Field>(Some(chrome), UiaRole::Edit, |ui| {
            ui.text(style, "");
        });
        let id = element.control_id(HitFlags::TEXT);
        let Target::Group(group) = element.node.target else {
            unreachable!()
        };
        let control = element.host.controls.get(id).unwrap();
        element.host.install_field(
            id,
            group,
            control.text.unwrap(),
            crate::text_input::InputScope::Default,
            control.scope,
            None,
        );
        element
            .host
            .field_binding(element.node.target.id(), id, source.into());
        element
            .height(Metric::RowH)
            .layout(|l| l.min_width = Some(Len::Times(Metric::RowH, 4.0)))
    }
    /// The control exists before its body; nested controls start a new part-ownership scope.
    pub fn control(
        &mut self,
        chrome: Option<Chrome>,
        role: UiaRole,
        children: impl FnOnce(&mut Ui<'_>),
    ) -> Element<'_> {
        self.control_as::<Any>(chrome, role, children)
    }

    pub(super) fn control_as<K>(
        &mut self,
        chrome: Option<Chrome>,
        role: UiaRole,
        children: impl FnOnce(&mut Ui<'_>),
    ) -> Element<'_, K> {
        let node = self.create(Preset::Row, false);
        let Target::Group(group) = node.target else {
            unreachable!()
        };
        let id = self.host.direct_control(
            group.node(),
            self.scope,
            role,
            HitFlags::INTERACTIVE | HitFlags::GESTURE,
        );
        let control = self.host.controls.get_mut(id).unwrap();
        control.front.hover_scope = self.hover_scope;
        if let Some(chrome) = chrome {
            self.host.declare_surface(group, chrome);
        }
        children(&mut Ui {
            host: self.host,
            members: self.members,
            parent: Some(group),
            after: None,
            scope: self.scope,
            owner: Some(id),
            hover_scope: self.hover_scope,
        });
        self.host.controls.get_mut(id).unwrap().dirty = true;
        Element {
            host: self.host,
            members: Some(self.members),
            node,
        }
        .layout(|layout| {
            layout.min_height = Some(Metric::RowH.into());
            layout.padding = Some([Metric::SpaceMd.into(), Metric::SpaceXs.into()]);
            layout.gap = Some(Metric::SpaceSm.into());
            layout.justify = Some(Align::Center);
        })
    }

    pub fn button(
        &mut self,
        chrome: Chrome,
        typography: TextStyle,
        text: impl Into<TextSource>,
    ) -> Element<'_> {
        let text = text.into();
        self.control(Some(chrome), UiaRole::Button, |ui| {
            if !match &text {
                TextSource::Static(text) => text.is_empty(),
                TextSource::Owned(text) => text.is_empty(),
                TextSource::Dynamic(_) => false,
            } {
                ui.text(typography, text);
            }
        })
    }

    pub fn surface(
        &mut self,
        chrome: Chrome,
        elevation: Elevation,
        children: impl FnOnce(&mut Ui<'_>),
    ) -> Element<'_> {
        let node = self.create(Preset::Stack, false);
        let Target::Group(group) = node.target else {
            unreachable!()
        };
        let scope = self.scope.elevate(elevation);
        self.host.styles.get_mut(group.node()).unwrap().scope = scope;
        self.host.declare_surface(group, chrome);
        children(&mut Ui {
            host: self.host,
            members: self.members,
            parent: Some(group),
            after: None,
            scope,
            owner: self.owner,
            hover_scope: self.hover_scope,
        });
        Element {
            host: self.host,
            members: Some(self.members),
            node,
        }
        .padding(Metric::SpaceLg)
    }
}

impl Host {
    pub(super) fn control_part(
        &mut self,
        owner: ControlId,
        part: Part,
        sprite: windows_scene::SpriteId,
    ) {
        let row = self
            .controls
            .get_mut(owner)
            .expect("the enclosing control is retained");
        if let Part::Trail { origin } = part {
            row.front.trail = Some((sprite.node(), origin));
        }
    }

    pub(super) fn control_scalar_part(
        &mut self,
        owner: ControlId,
        node: NodeId,
        part: crate::widget::ScalarPart,
    ) {
        assert_eq!(
            self.mounts.get(node).unwrap().channels & (1 << part.property() as u8),
            0,
            "a scalar part cannot also bind its driven property"
        );
        let row = self
            .controls
            .get_mut(owner)
            .expect("the enclosing control is retained");
        let parts = &mut row.front.scalar_parts;
        let index = parts
            .iter()
            .position(|entry| entry.is_some_and(|(held, _)| held == node))
            .or_else(|| parts.iter().position(Option::is_none))
            .expect("a scalar supports at most four parts");
        parts[index] = Some((node, part));
    }

    pub(super) fn direct_control(
        &mut self,
        node: NodeId,
        scope: Scope,
        role: UiaRole,
        flags: HitFlags,
    ) -> ControlId {
        let flags = flags
            | if self.mounts.get(node).unwrap().no_inflate {
                HitFlags::NO_INFLATE
            } else {
                HitFlags::NONE
            }
            | if role == UiaRole::None {
                HitFlags::NONE
            } else {
                HitFlags::UIA
            };
        let id = self.mint_control(ControlRow {
            uia: role,
            ..ControlRow::new(node, scope)
        });
        let hit = HitDecl {
            id,
            flags,
            touch_inflate: None,
        };
        let row = self.controls.get_mut(id).unwrap();
        row.hit = Some(hit);
        row.front.id = id;
        row.front.hover = 0.06;
        row.front.press = 0.12;
        self.mounts.get_mut(node).unwrap().control = Some(id);
        self.model.hit(node, Some(hit));
        if flags.contains(HitFlags::GESTURE) {
            self.gestures
                .push((id, crate::gesture::GestureDecl::default()));
        }
        id
    }

    pub(super) fn direct_state(&mut self, id: ControlId) {
        let row = self.controls.get(id).unwrap();
        if let Some(mut hit) = row.hit {
            if row.disabled {
                hit.flags = if hit.flags.contains(HitFlags::UIA) {
                    HitFlags::UIA
                } else {
                    HitFlags::NONE
                };
            }
            self.model.hit(row.node, Some(hit));
        }
        self.set_state(
            id,
            Some(if row.disabled {
                ModelState::Disabled
            } else if row.selected {
                ModelState::Selected
            } else {
                ModelState::Rest
            }),
        );
    }
}

impl<K> Element<'_, K> {
    pub fn thumb(self) -> Self {
        let id = self
            .node
            .owner
            .expect("a thumb requires an enclosing control");
        let row = self.host.controls.get_mut(id).unwrap();
        row.front.thumb = Some(self.node.target.id());
        row.dirty = true;
        self
    }
    pub fn scalar_part(self, part: crate::widget::ScalarPart) -> Self {
        let id = self
            .node
            .owner
            .expect("a scalar part requires an enclosing control");
        let node = self.node.target.id();
        self.host.control_scalar_part(id, node, part);
        let row = self.host.controls.get_mut(id).unwrap();
        row.dirty = true;
        self
    }
    pub(super) fn control_id(&mut self, flags: HitFlags) -> ControlId {
        let node = self.node.target.id();
        let id = if let Some(id) = self.host.mounts.get(node).unwrap().control {
            id
        } else {
            let id = self.host.direct_control(
                node,
                self.host.styles.get(node).unwrap().scope,
                UiaRole::None,
                flags,
            );
            self.host.controls.get_mut(id).unwrap().front.hover_scope = self.node.hover_scope;
            id
        };
        if let Some(mut hit) = self.host.controls.get(id).unwrap().hit {
            hit.flags = hit.flags | flags;
            self.host.controls.get_mut(id).unwrap().hit = Some(hit);
            self.host.direct_state(id);
        }
        id
    }

    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        let id = self.control_id(HitFlags::INTERACTIVE | HitFlags::GESTURE);
        if let Some(old) = self
            .host
            .controls
            .get_mut(id)
            .unwrap()
            .click
            .replace(Rc::new(callback))
        {
            self.host.retired.push(Retired::Click(old));
        }
        self
    }

    /// Literal names stay borrowed; generated names are released with the control.
    pub fn name(mut self, name: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        let id = self.control_id(HitFlags::UIA);
        self.host.controls.get_mut(id).unwrap().name = Some(name.into());
        self.host.uia_restale();
        self
    }

    pub fn key(mut self, key: &'static str) -> Self {
        let id = self.control_id(HitFlags::NONE);
        self.host.controls.get_mut(id).unwrap().key = Some(key);
        self
    }

    pub fn disabled<M>(mut self, value: impl Signal<bool, M> + 'static) -> Self {
        let id = self.control_id(HitFlags::NONE);
        self.state_binding(id, Destination::Disabled, value, |row, value| {
            row.disabled = value
        })
    }

    pub fn selected<M>(mut self, value: impl Signal<bool, M> + 'static) -> Self {
        let id = self.control_id(HitFlags::NONE);
        if let Target::Group(group) = self.node.target {
            self.host.surface_selectable(group);
        }
        self.state_binding(id, Destination::Selected, value, |row, value| {
            row.selected = value
        })
    }

    fn state_binding<M>(
        self,
        id: ControlId,
        destination: Destination,
        value: impl Signal<bool, M> + 'static,
        write: fn(&mut ControlRow, bool),
    ) -> Self {
        let node = self.node.target.id();
        if value.is_constant() {
            self.host.replace_binding(node, destination);
            write(self.host.controls.get_mut(id).unwrap(), value.read());
            self.host.direct_state(id);
        } else {
            self.host.bind_to(node, destination, move || {
                let value = value.read();
                Host::with(|host| {
                    write(host.controls.get_mut(id).unwrap(), value);
                    host.direct_state(id);
                });
            });
        }
        self
    }

    pub fn wash(mut self, wash: Wash) -> Self {
        self.control_id(HitFlags::INTERACTIVE);
        if let Target::Group(group) = self.node.target {
            self.host.surface_wash(group, wash);
        }
        self
    }
}

impl Element<'_, Scalar> {
    pub fn on_change(mut self, callback: impl Fn(f64) + 'static) -> Self {
        let id = self.control_id(HitFlags::GESTURE);
        if let Some(old) = self
            .host
            .controls
            .get_mut(id)
            .unwrap()
            .change
            .replace(Rc::new(callback))
        {
            self.host.retired.push(Retired::Change(old));
        }
        self
    }
    pub fn on_commit(mut self, callback: impl Fn(f64) + 'static) -> Self {
        let id = self.control_id(HitFlags::GESTURE);
        if let Some(old) = self
            .host
            .controls
            .get_mut(id)
            .unwrap()
            .commit
            .replace(Rc::new(callback))
        {
            self.host.retired.push(Retired::Change(old));
        }
        self
    }
    pub fn on_cancel(mut self, callback: impl Fn() + 'static) -> Self {
        let id = self.control_id(HitFlags::GESTURE);
        if let Some(old) = self
            .host
            .controls
            .get_mut(id)
            .unwrap()
            .cancel
            .replace(Rc::new(callback))
        {
            self.host.retired.push(Retired::Click(old));
        }
        self
    }
}

impl Element<'_, super::Field> {
    pub fn on_commit(mut self, callback: impl Fn(&str) + 'static) -> Self {
        let id = self.control_id(HitFlags::TEXT);
        if let Some(old) = self
            .host
            .fields
            .get_mut(id)
            .unwrap()
            .callback
            .replace(Rc::new(callback))
        {
            self.host.retired.push(Retired::Text(old));
        }
        self
    }
    pub fn scope(mut self, scope: crate::text_input::InputScope) -> Self {
        let id = self.control_id(HitFlags::TEXT);
        self.host.fields.get_mut(id).unwrap().scope = scope;
        for source in &mut self.host.field_sources {
            if source.id == id {
                source.scope = scope;
            }
        }
        self
    }
}

impl<'a> Element<'a> {
    pub fn turn<M>(
        self,
        value: impl Signal<f64, M> + 'static,
        range: crate::widget::Range,
    ) -> Element<'a, Scalar> {
        self.scalar_source(crate::widget::Interaction::Turn(range), value, |value| {
            crate::widget::ScalarValue { value, epoch: 0 }
        })
    }
    pub fn turn_source<M>(
        self,
        value: impl Signal<crate::widget::ScalarValue, M> + 'static,
        range: crate::widget::Range,
    ) -> Element<'a, Scalar> {
        self.scalar_source(crate::widget::Interaction::Turn(range), value, |value| {
            value
        })
    }
    pub fn slide<M>(
        self,
        value: impl Signal<f64, M> + 'static,
        range: crate::widget::Range,
    ) -> Element<'a, Scalar> {
        self.scalar_source(crate::widget::Interaction::Slide(range), value, |value| {
            crate::widget::ScalarValue { value, epoch: 0 }
        })
    }
    fn scalar_source<T: Copy + 'static, M>(
        mut self,
        drive: crate::widget::Interaction,
        value: impl Signal<T, M> + 'static,
        map: impl Fn(T) -> crate::widget::ScalarValue + 'static,
    ) -> Element<'a, Scalar> {
        let id = self.control_id(HitFlags::GESTURE | HitFlags::INTERACTIVE | HitFlags::UIA);
        let row = self.host.controls.get_mut(id).unwrap();
        row.uia = UiaRole::Slider;
        row.front.drive = Some(drive);
        row.dirty = true;
        let range = match drive {
            crate::widget::Interaction::Turn(range) | crate::widget::Interaction::Slide(range) => {
                range
            }
            _ => unreachable!(),
        };
        let gesture = match drive {
            crate::widget::Interaction::Slide(range) => {
                crate::gesture::GestureDecl::slider(range.vertical)
            }
            _ => crate::gesture::GestureDecl {
                drag: Some(crate::gesture::DragDecl::turn()),
                ..Default::default()
            },
        };
        self.host.gestures.push((id, gesture));
        let node = self.node.target.id();
        if value.is_constant() {
            let value = map(value.read());
            self.host.replace_binding(node, Destination::Scalar);
            self.host
                .publish_fraction(id, range.fraction(value.value), value.epoch);
        } else {
            self.host.bind_to(node, Destination::Scalar, move || {
                let value = map(value.read());
                Host::with(|h| h.publish_fraction(id, range.fraction(value.value), value.epoch));
            });
        }
        Element {
            host: self.host,
            members: self.members,
            node: super::Node {
                target: self.node.target,
                runtime: self.node.runtime,
                owner: self.node.owner,
                hover_scope: self.node.hover_scope,
                kind: core::marker::PhantomData,
            },
        }
    }
}
impl<K> Element<'_, K> {
    pub(crate) fn hit(mut self, flags: HitFlags, role: UiaRole) -> Self {
        let id = self.control_id(
            flags
                | if role == UiaRole::None {
                    HitFlags::NONE
                } else {
                    HitFlags::UIA
                },
        );
        let row = self.host.controls.get_mut(id).unwrap();
        row.uia = role;
        row.front.hover_scope = if row.front.hover_scope == Some(id) {
            Some(id)
        } else {
            self.node.hover_scope
        };
        row.dirty = true;
        self
    }
    pub fn no_inflate(mut self) -> Self {
        self.host
            .mounts
            .get_mut(self.node.target.id())
            .unwrap()
            .no_inflate = true;
        if self
            .host
            .mounts
            .get(self.node.target.id())
            .unwrap()
            .control
            .is_some()
        {
            self.control_id(HitFlags::NO_INFLATE);
        }
        self
    }
    pub fn caption(mut self, button: windows_window::CaptionButton) -> Self {
        let id = self.control_id(HitFlags::INTERACTIVE);
        self.host.caption.set(button, id);
        self
    }
    pub fn hover_scope(mut self, hovered: crate::signal::Cell<bool>) -> Self {
        let id = self.control_id(HitFlags::INTERACTIVE);
        let row = self.host.controls.get_mut(id).unwrap();
        row.hovered = Some(hovered);
        row.front.observes_hover = true;
        row.front.hover_scope = Some(id);
        row.dirty = true;
        self
    }
    /// Groups hover, press and keyboard focus for one retained reveal target.
    /// Declare the scope before mounting its children.
    pub fn interaction_scope(mut self) -> Self {
        let id = self.control_id(HitFlags::GESTURE);
        let row = self.host.controls.get_mut(id).unwrap();
        row.front.hover_scope = Some(id);
        row.dirty = true;
        self
    }

    /// Reveals this element while its enclosing interaction scope is active.
    /// Each scope accepts one target, which must be mounted below the scope.
    /// The front thread owns its opacity; layout and hit testing remain active.
    pub fn reveal_on_interaction(self) -> Self {
        let scope = self
            .node
            .hover_scope
            .expect("a reveal requires an interaction scope");
        let node = self.node.target.id();
        let row = self.host.controls.get_mut(scope).unwrap();
        assert!(
            row.front.reveal.is_none() || row.front.reveal == node,
            "one reveal target per scope"
        );
        if row.front.reveal == node {
            return self;
        }
        assert!(
            self.host.mounts.get(node).unwrap().channels
                & (1 << windows_scene::Prop::Opacity as u8)
                == 0,
            "an interaction reveal requires unclaimed opacity"
        );
        let row = self.host.controls.get_mut(scope).unwrap();
        row.front.reveal = node;
        row.dirty = true;
        self.host.set_channel(
            node,
            node,
            windows_scene::Prop::Opacity,
            windows_scene::Value::Scalar(0.0),
        );
        self
    }
    pub fn on_unhandled_escape(self, callback: impl Fn() + 'static) -> Self {
        self.host
            .set_escape(self.node.target.id(), Rc::new(callback));
        self
    }
    pub fn drag(mut self, decl: crate::gesture::DragDecl) -> Self {
        let id = self.control_id(HitFlags::GESTURE);
        self.host
            .gestures
            .push((id, crate::gesture::GestureDecl::default().with_drag(decl)));
        self
    }
    pub fn on_drag(
        mut self,
        decl: crate::gesture::DragDecl,
        callback: impl Fn(crate::widget::Dragging) + 'static,
    ) -> Self {
        let id = self.control_id(HitFlags::GESTURE);
        if let Some(old) = self
            .host
            .controls
            .get_mut(id)
            .unwrap()
            .drag
            .replace(Rc::new(callback))
        {
            self.host.retired.push(Retired::Drag(old));
        }
        self.drag(decl)
    }
    pub fn gesture(mut self, decl: crate::gesture::GestureDecl) -> Self {
        let id = self.control_id(HitFlags::GESTURE);
        self.host.gestures.push((id, decl));
        self
    }
    pub fn tip(self, text: impl Into<TextSource>) -> Self {
        self.tip_at(crate::overlay::Side::Bottom, text)
    }
    pub fn tip_at(mut self, side: crate::overlay::Side, text: impl Into<TextSource>) -> Self {
        let id = self.control_id(HitFlags::INTERACTIVE);
        if let Some((old, _)) = self
            .host
            .controls
            .get_mut(id)
            .unwrap()
            .tip
            .replace((Rc::new(text.into()), side))
        {
            self.host.retired.push(Retired::Tip(old));
        }
        self
    }
    pub fn flyout(mut self, body: impl Fn(&mut Ui<'_>) + 'static) -> Self {
        let id = self.control_id(HitFlags::GESTURE | HitFlags::INTERACTIVE);
        if let Some(old) = self
            .host
            .controls
            .get_mut(id)
            .unwrap()
            .flyout
            .replace(Rc::new(body))
        {
            self.host.retired.push(Retired::Flyout(old));
        }
        self
    }
    pub fn popup_when<M>(
        self,
        shown: impl Signal<bool, M> + 'static,
        spec: crate::overlay::Spec,
        closed: impl Fn() + 'static,
        body: impl Fn(&mut Ui<'_>) + 'static,
    ) -> Self {
        let node = self.node.target.id();
        self.host.mounts.get_mut(node).unwrap().popup = true;
        let body = Rc::new(body);
        let closed = Rc::new(closed);
        if shown.is_constant() {
            self.host.replace_binding(node, Destination::Popup);
            let request = if shown.read() {
                crate::overlay::Request::Show {
                    key: node,
                    spec,
                    body,
                    closed,
                }
            } else {
                self.host.retired.push(Retired::Flyout(body));
                self.host.retired.push(Retired::Click(closed));
                crate::overlay::Request::Close(node)
            };
            self.host.request_popup(request);
            return self;
        }
        self.host.bind_to(node, Destination::Popup, move || {
            let request = if shown.read() {
                crate::overlay::Request::Show {
                    key: node,
                    spec,
                    body: body.clone(),
                    closed: closed.clone(),
                }
            } else {
                crate::overlay::Request::Close(node)
            };
            Host::with(|h| h.request_popup(request));
        });
        self
    }
    pub fn validation(mut self, read: impl Fn() -> Option<&'static str> + 'static) -> Self {
        let id = self.control_id(HitFlags::UIA);
        let node = self.node.target.id();
        self.host.bind_to(node, Destination::Validation, move || {
            let value = read();
            Host::with(|h| {
                h.controls.get_mut(id).unwrap().validation = value;
                h.direct_state(id);
                h.uia_restale();
            });
        });
        self
    }
}
