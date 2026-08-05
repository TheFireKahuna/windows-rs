//! Drives UI Automation against a real window, so an automation client can walk the tree.
//!
//! An attached client exercises what no unit test reaches: that `WM_GETOBJECT` returns a
//! provider `UiaReturnRawElementProvider` accepts, that the fragment root is reachable, that
//! the control types read as intended, and that a raised event reaches a listener.
//!
//! ```text
//! cargo run -p windows-ui --example uia
//! ```
//!
//! Attach Accessibility Insights or Inspect and walk the tree. Press `T` to toggle the check
//! box, `←`/`→` to move the slider, `Tab` to move focus, and `Q` to finish. Invoking the
//! button, setting the slider's value and toggling the box each arrive as an [`Action`] and
//! are applied in the same tick, so a keystroke here and a screen reader take one code path.
//!
//! No scene is built: the hit array and the seeds are written out by hand, because
//! automation resolves through the same flat hit array whatever produced it.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use windows_present::{Rect, RegionParts, SubId};
use windows_scene::{ControlId, HitEntry, HitFlags, HitTable, Ids, NO_ENTRY, NodeId};
use windows_ui::Result;
use windows_ui::uia::{
    Action, ColFlags, PartDecl, RegionPeer, Seed, Seeds, State, Text, Uia, Value,
};
use windows_ui::widget::{ModelState, Range, UiaRole};
use windows_window::Window;

/// The screen, in DIPs: a group holding a button, a check box, a label and a slider, and a
/// graph beside it.
const LAYOUT: [(&str, UiaRole, f32, f32, f32, f32); 6] = [
    ("Output", UiaRole::Group, 16.0, 16.0, 464.0, 132.0),
    ("Mute", UiaRole::Button, 32.0, 40.0, 152.0, 76.0),
    ("Bypass", UiaRole::CheckBox, 168.0, 40.0, 288.0, 76.0),
    // A text run, then a control with no text of its own: the slider takes its name from the
    // label beside it and reports the relation through `LabeledBy`.
    ("Gain", UiaRole::Text, 32.0, 92.0, 96.0, 112.0),
    ("", UiaRole::Slider, 104.0, 92.0, 464.0, 124.0),
    ("Spectrum", UiaRole::Graph, 16.0, 164.0, 464.0, 324.0),
];

const GROUP: usize = 0;
const MUTE: usize = 1;
const BYPASS: usize = 2;
/// Indexes the text run whose name the slider beside it adopts.
#[expect(dead_code, reason = "names the layout row rather than indexing it")]
const LABEL: usize = 3;
const GAIN: usize = 4;
const SPECTRUM: usize = 5;

const GAIN_RANGE: Range = Range::new(-60.0, 12.0);

/// Holds the screen's state and everything a client can change through automation.
struct Model {
    ids: Vec<ControlId>,
    gain: f64,
    bypassed: bool,
    muted: bool,
    /// Distance the renderer has moved its bands, standing in for a mapping change.
    spread: f32,
}

fn main() -> Result<()> {
    let mut authority = Ids::new();
    let mut model = Model {
        ids: (0..LAYOUT.len()).map(|_| authority.mint()).collect(),
        gain: 0.0,
        bypassed: false,
        muted: false,
        spread: 0.0,
    };

    let uia = Rc::new(RefCell::new(Uia::new()));
    let quit = Rc::new(std::cell::Cell::new(false));

    let window = Window::new("windows-ui — UI Automation")
        .size_dips(496.0, 360.0)
        .on_message({
            let uia = Rc::clone(&uia);
            let quit = Rc::clone(&quit);
            move |_, message, wparam, lparam| {
                // The only automation call that arrives on the pump. Everything a client asks
                // after this is served off this thread.
                if message == WM_GETOBJECT {
                    return uia.borrow_mut().get_object(wparam, lparam);
                }
                // Detached while the handle is still valid: automation caches per window, and
                // by `Drop` the handle names no window to clear.
                if message == WM_DESTROY {
                    uia.borrow_mut().detach();
                }
                if message == WM_KEYDOWN && wparam == b'Q' as usize {
                    quit.set(true);
                }
                None
            }
        })
        .create()?;

    let pacer = window.pacer()?;
    uia.borrow_mut().attach(window.hwnd());

    let mut hits = HitTable::default();
    hits.replace(&entries(&model.ids));

    // A band handle exists only as pixels inside the graph, so it is a part: nameable,
    // value-reporting and reachable by element-from-point, with no visual behind it.
    //
    // The two halves arrive from different places. `RegionParts` carries the versioned
    // geometry a renderer publishes, standing in here for the present thread; the names and
    // roles are declared once on this side. `sync_regions` joins them each tick.
    let geometry = Arc::new(RegionParts::new());
    let levels: Arc<[AtomicU64]> = Arc::from([
        AtomicU64::new((-3.0f64).to_bits()),
        AtomicU64::new(1.5f64.to_bits()),
        AtomicU64::new((-8.0f64).to_bits()),
    ]);
    uia.borrow_mut().watch_region(RegionPeer {
        id: model.ids[SPECTRUM],
        geometry: Arc::clone(&geometry),
        parts: vec![
            PartDecl::new(0, "Low band", UiaRole::Slider),
            PartDecl::new(1, "Mid band", UiaRole::Slider),
            PartDecl::new(2, "High band", UiaRole::Slider),
        ],
        values: Some(Arc::clone(&levels)),
    });
    publish_bands(&geometry, 0.0);

    let mut seeds = Seeds::default();
    publish(&uia, &hits, &mut seeds, &model, &window);

    println!("attach Accessibility Insights or Inspect and walk the tree");
    println!("T toggles Bypass · ← → move Gain · Q quits\n");

    let mut actions = Vec::new();
    let mut message = MSG::default();
    loop {
        // SAFETY: `message` is a stack local valid for writes for the whole call, and a null
        // window handle is the documented request for every message on this thread.
        let more = unsafe { GetMessageW(&mut message, core::ptr::null_mut(), 0, 0) };
        if more.0 <= 0 || quit.get() {
            break;
        }
        // SAFETY: `GetMessageW` returned a positive result above, so `message` is fully
        // initialized, and both calls take it by shared reference.
        unsafe {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        // A keystroke stands in for the widget layer this example does not build: it changes
        // the model as an application would, and automation learns about the change through
        // the same calls a front thread makes.
        if message.message == WM_KEYDOWN {
            // The keystroke asks for the tick a front thread's own sources would have asked
            // for. A presenting region's epoch drives the frame clock continuously, so a
            // window with one never posts this itself.
            // SAFETY: `hwnd` names a window this process owns, and `WM_FRAME` carries no
            // pointer in either parameter.
            unsafe {
                _ = PostMessageW(window.hwnd(), windows_window::WM_FRAME, 0, 0);
            }
            let (gain, bypassed) = (model.gain, model.bypassed);
            match message.wParam as u16 {
                // A republish, so a client that is already attached can observe one.
                0x52 => publish(&uia, &hits, &mut seeds, &model, &window), // R
                // Moves the renderer's mapping, which is what a band drag looks like from
                // here; the join carries it without republishing the tree.
                0x42 => {
                    model.spread += 24.0;
                    levels[0].store((-3.0 - f64::from(model.spread) * 0.1).to_bits(), Relaxed);
                    publish_bands(&geometry, model.spread);
                    println!("bands moved by {}", model.spread);
                } // B
                0x54 => set_bypassed(&uia, &mut model, !bypassed), // T
                0x25 => set_gain(&uia, &mut model, gain - 3.0),    // ←
                0x27 => set_gain(&uia, &mut model, gain + 3.0),    // →
                _ => {}
            }
        }

        if message.message != windows_window::WM_FRAME {
            continue;
        }

        // The tick. Five steps, in this order, every time.
        //
        // 1. A client that has just attached is looking at nothing until the first publish,
        //    and an idle window has no other reason to make one.
        if uia.borrow().wants_tree() {
            publish(&uia, &hits, &mut seeds, &model, &window);
        }

        // 2. What clients asked for since the last tick. Applied here, on the thread that
        //    owns the model, which is why the provider was free to return immediately.
        actions.clear();
        uia.borrow_mut().drain(&mut actions);
        for action in &actions {
            match *action {
                Action::Invoke(id) if id == model.ids[MUTE] => {
                    model.muted = !model.muted;
                    println!("invoked Mute → {}", model.muted);
                    // Raised here rather than by the provider: the provider returned before
                    // this ran, as the Invoke pattern requires of it.
                    uia.borrow_mut().invoked(id);
                    uia.borrow_mut().set_model(
                        id,
                        if model.muted {
                            ModelState::Selected
                        } else {
                            ModelState::Rest
                        },
                    );
                }
                Action::Toggle(id) if id == model.ids[BYPASS] => {
                    let flipped = !model.bypassed;
                    set_bypassed(&uia, &mut model, flipped);
                }
                Action::SetValue(id, v) if id == model.ids[GAIN] => {
                    set_gain(&uia, &mut model, v);
                }
                Action::Focus(id) => {
                    println!("client asked for focus on {}", label(&model, id));
                    uia.borrow_mut().set_focus(Some(id));
                }
                other => println!("action: {other:?}"),
            }
        }

        // 3. Re-joins any watched region whose renderer has moved. A region that did not move
        //    costs one acquire load, so this runs unconditionally.
        uia.borrow_mut().sync_regions();

        // 4. The window may have moved, and automation speaks screen pixels.
        let scale = window.scale().unwrap_or(1.0);
        let origin = client_origin(window.hwnd());
        uia.borrow_mut().set_window(origin, scale);

        // 5. Raised last, so the tree a client reads back is the one the event describes,
        //    and so no raise re-enters a handler that is still running.
        uia.borrow_mut().flush();
    }

    println!("\npacer: {:?}", pacer.health());
    Ok(())
}

/// Rebuilds the accessible tree from the hit array and the model.
///
/// A front thread runs this where the patch is applied, taking seeds from `Host::uia_seeds`,
/// which synthesises them from what each widget declared. The seeds built here are the same
/// data written out by hand.
fn publish(
    uia: &Rc<RefCell<Uia>>,
    hits: &HitTable,
    seeds: &mut Seeds,
    model: &Model,
    window: &Window,
) {
    seeds.clear();
    for (at, (name, role, ..)) in LAYOUT.iter().enumerate() {
        let name = seeds.intern(name);
        seeds.rows.push(Seed {
            id: model.ids[at],
            role: *role,
            name,
            help: Text::default(),
            key: None,
            value: match *role {
                UiaRole::Slider => Value::Range(GAIN_RANGE),
                UiaRole::Graph => Value::Range(Range::new(-60.0, 12.0)),
                UiaRole::Text => Value::Text,
                _ => Value::None,
            },
            flags: match *role {
                UiaRole::Group | UiaRole::Text => ColFlags::NONE,
                UiaRole::Graph => ColFlags::FOCUSABLE | ColFlags::LIVE_POLITE,
                _ => ColFlags::FOCUSABLE,
            },
            state: State::ENABLED,
        });
    }
    seeds.sort();

    let mut uia = uia.borrow_mut();
    uia.publish(hits.entries(), seeds);
    uia.set_window(client_origin(window.hwnd()), window.scale().unwrap_or(1.0));
    uia.set_value(model.ids[GAIN], model.gain);
    uia.set_state(model.ids[BYPASS], State::TOGGLED, model.bypassed);
}

fn set_gain(uia: &Rc<RefCell<Uia>>, model: &mut Model, to: f64) {
    model.gain = to.clamp(GAIN_RANGE.min, GAIN_RANGE.max);
    println!("gain → {:.1} dB", model.gain);
    uia.borrow_mut().set_value(model.ids[GAIN], model.gain);
}

fn set_bypassed(uia: &Rc<RefCell<Uia>>, model: &mut Model, to: bool) {
    model.bypassed = to;
    println!("bypass → {}", model.bypassed);
    uia.borrow_mut()
        .set_state(model.ids[BYPASS], State::TOGGLED, model.bypassed);
}

fn label(model: &Model, id: ControlId) -> &'static str {
    model
        .ids
        .iter()
        .position(|&key| key == id)
        .map_or("?", |at| LAYOUT[at].0)
}

/// Publishes where each band sits, in region-local DIPs, as a renderer would.
fn publish_bands(geometry: &RegionParts, spread: f32) {
    let at = |sub: u32, x: f32| windows_present::Part {
        id: SubId(sub),
        rect: Rect::new(x + spread, 16.0, x + spread + 72.0, 144.0),
    };
    geometry.publish(&[at(0, 24.0), at(1, 200.0), at(2, 376.0)]);
}

/// Builds the hit array by hand.
///
/// Automation's fragment navigation reads the `parent` column, so the group's four children
/// name the group, and the group and the graph name no parent.
fn entries(ids: &[ControlId]) -> Vec<HitEntry> {
    LAYOUT
        .iter()
        .enumerate()
        .map(|(at, &(_, role, x0, y0, x1, y1))| HitEntry {
            x0,
            y0,
            x1,
            y1,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: match at {
                GROUP | SPECTRUM => NO_ENTRY,
                _ => GROUP as u32,
            },
            flags: match role {
                // A run routes no pointer and takes no focus; it is an element and
                // nothing more, which the scan skips on one flags test.
                UiaRole::Group | UiaRole::Text => HitFlags::UIA,
                _ => HitFlags::INTERACTIVE | HitFlags::UIA,
            },
            scroll_src: NodeId::NONE,
            id: ids[at],
        })
        .collect()
}

/// Returns the client area's top-left corner, in physical pixels.
fn client_origin(hwnd: *mut core::ffi::c_void) -> windows_numerics::Vector2 {
    let mut point = [0i32; 2];
    // SAFETY: `hwnd` names a window this process owns, and `point` is a stack local valid for
    // writes for the whole call.
    unsafe {
        _ = ClientToScreen(hwnd, &raw mut point);
    }
    windows_numerics::Vector2 {
        x: point[0] as f32,
        y: point[1] as f32,
    }
}

const WM_GETOBJECT: u32 = 0x003D;
const WM_KEYDOWN: u32 = 0x0100;
const WM_DESTROY: u32 = 0x0002;

/// Mirrors the platform's message record. The unread fields are declared because the layout
/// is the ABI.
#[repr(C)]
#[derive(Clone, Copy, Default)]
#[expect(
    non_snake_case,
    clippy::upper_case_acronyms,
    reason = "the platform's own name, field names and layout"
)]
struct MSG {
    _hwnd: *mut core::ffi::c_void,
    message: u32,
    wParam: usize,
    _lParam: isize,
    _time: u32,
    _pt: [i32; 2],
}

windows_core::link!("user32.dll" "system" fn GetMessageW(msg: *mut MSG, hwnd: *mut core::ffi::c_void, min: u32, max: u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TranslateMessage(msg: *const MSG) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn DispatchMessageW(msg: *const MSG) -> isize);
windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd: *mut core::ffi::c_void, msg: u32, w: usize, l: isize) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ClientToScreen(hwnd: *mut core::ffi::c_void, point: *mut [i32; 2]) -> windows_core::BOOL);
