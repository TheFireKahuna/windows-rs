use super::*;
use std::cell::{Cell, RefCell};

/// Which GPU device a [`surface_painter`] draws with. Mirrors Win2D's
/// `UseSharedDevice` / `ForceSoftwareRenderer` / `CustomDevice` knobs.
#[derive(Clone, Default)]
pub enum DeviceSource {
    /// Use a process-thread-wide shared device (Win2D's `UseSharedDevice = true`,
    /// the default). One device backs every canvas on the UI thread — far less GPU
    /// memory and driver overhead than a device per control. When it is lost it is
    /// recreated once and every dependent control gets a `NewDevice`
    /// [`create_resources`](SurfacePainterBuilder::create_resources).
    #[default]
    Shared,
    /// Create a fresh device used only by this control (`UseSharedDevice = false`).
    Owned,
    /// Draw with a device the caller already owns — share an existing
    /// [`GpuDevice`] (e.g. one also driving an [`animated_canvas`] swap chain).
    /// The caller owns its lifetime; if it is lost, recovery is the caller's
    /// responsibility (re-render with a fresh device).
    Custom(GpuDevice),
}

// A process-thread-wide shared `GpuDevice` (Win2D's shared device), kept separate
// for hardware and forced-software so `ForceSoftwareRenderer` controls never share
// a device with hardware ones. Each slot caches the device with a monotonic
// generation; resetting clears it so the next acquire makes a fresh one with a new
// generation, which dependent controls observe as a `NewDevice`.
thread_local! {
    static SHARED_HW: RefCell<Option<(GpuDevice, u64)>> = const { RefCell::new(None) };
    static SHARED_SW: RefCell<Option<(GpuDevice, u64)>> = const { RefCell::new(None) };
    static NEXT_DEVICE_GEN: Cell<u64> = const { Cell::new(1) };
}

fn next_device_gen() -> u64 {
    NEXT_DEVICE_GEN.with(|g| {
        let v = g.get();
        g.set(v.wrapping_add(1));
        v
    })
}

fn make_device(force_software: bool) -> Result<GpuDevice> {
    if force_software {
        GpuDevice::new_warp()
    } else {
        GpuDevice::new_or_warp()
    }
}

/// Get (creating if needed) the shared device for the given renderer, with its
/// generation.
fn shared_device(force_software: bool) -> Option<(GpuDevice, u64)> {
    let get = |slot: &RefCell<Option<(GpuDevice, u64)>>| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let device = make_device(force_software).ok()?;
            *slot = Some((device, next_device_gen()));
        }
        slot.clone()
    };
    if force_software {
        SHARED_SW.with(get)
    } else {
        SHARED_HW.with(get)
    }
}

/// Drop the cached shared device so the next [`shared_device`] makes a fresh one.
fn reset_shared_device(force_software: bool) {
    if force_software {
        SHARED_SW.with(|s| *s.borrow_mut() = None);
    } else {
        SHARED_HW.with(|s| *s.borrow_mut() = None);
    }
}

/// Acquire the device for a control per its [`DeviceSource`], returning it with a
/// generation that changes whenever the underlying device is recreated.
pub(crate) fn acquire_device(
    source: &DeviceSource,
    force_software: bool,
    owned: &HookRef<Option<GpuDevice>>,
    owned_gen: &HookRef<u64>,
) -> Option<(GpuDevice, u64)> {
    match source {
        DeviceSource::Custom(device) => Some((device.clone(), 0)),
        DeviceSource::Shared => shared_device(force_software),
        DeviceSource::Owned => {
            let mut slot = owned.borrow_mut();
            if slot.is_none() {
                *slot = make_device(force_software).ok();
                if slot.is_some() {
                    owned_gen.set(next_device_gen());
                }
            }
            slot.clone().map(|d| (d, owned_gen.get_cloned()))
        }
    }
}

/// Drop the control's device after a loss so the next acquire makes a fresh one.
/// A `Custom` device is caller-owned and left untouched.
pub(crate) fn reset_device(
    source: &DeviceSource,
    force_software: bool,
    owned: &HookRef<Option<GpuDevice>>,
) {
    match source {
        DeviceSource::Custom(_) => {}
        DeviceSource::Shared => reset_shared_device(force_software),
        DeviceSource::Owned => *owned.borrow_mut() = None,
    }
}
