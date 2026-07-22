//! Pure conversion helpers between reactor's backend-facing types and
//! the underlying `Microsoft.UI.Xaml` types. These are free functions
//! that touch neither `WinUIBackend` state nor the `Handle` enum — see
//! `winui/backend/mod.rs` for the dispatch tables and `Handle`-aware
//! helpers.

use super::*;

/// A [`CanvasOffset`] in DIPs. `Canvas.Left`/`Top` take pixels, so a fraction
/// has no containing extent to resolve against and degrades to the edge.
pub(super) fn canvas_dips(v: CanvasOffset) -> f64 {
    match v {
        CanvasOffset::Dip(x) => x,
        CanvasOffset::Fraction(_) => 0.0,
    }
}

pub(super) fn to_xaml_gridlength(v: GridLength) -> Result<bindings::GridLength> {
    use bindings::GridUnitType;
    match v {
        GridLength::Auto => Ok(bindings::GridLength {
            value: 0.0,
            grid_unit_type: GridUnitType::Auto,
        }),
        GridLength::Pixel(v) => Ok(bindings::GridLength {
            value: v,
            grid_unit_type: GridUnitType::Pixel,
        }),
        GridLength::Star(v) => Ok(bindings::GridLength {
            value: v,
            grid_unit_type: GridUnitType::Star,
        }),
        // A XAML `Grid` has a fixed track count — no unit type solves one from
        // the container's width, and the wrapping panels that come close
        // (`VariableSizedWrapGrid`, `ItemsWrapGrid`) are uniform-cell, not
        // content-flow. So this collapses to a single full-width track: the
        // children fall back on whatever placement they carry. A XAML `Grid`
        // also has no auto-flow, so the auto-placement `AutoFill` relies on is
        // DComp-only: a panel that uses it should set explicit rows too if it
        // must render on this backend.
        GridLength::AutoFill(_) => Ok(bindings::GridLength {
            value: 1.0,
            grid_unit_type: GridUnitType::Star,
        }),
    }
}

/// Encode a reactor [`Color`] (linear scRGB) into a WinRT `Windows.UI.Color`
/// (8-bit sRGB, the ABI `Microsoft.UI.Xaml` consumes). This is the SDR-fallback
/// boundary: the linear channels are clamped to `[0, 1]` and sRGB-encoded so the
/// WinUI backend renders visually identical to the FP16 path's SDR appearance.
pub(super) fn to_winrt(c: Color) -> bindings::Color {
    let (r, g, b, a) = c.to_srgb8();
    bindings::Color { a, r, g, b }
}

pub(super) fn solid_brush(c: Color) -> Result<bindings::SolidColorBrush> {
    let brush = bindings::SolidColorBrush::new()?;
    brush.SetColor(to_winrt(c))?;
    Ok(brush)
}

pub(super) fn string_as_textblock(s: &str) -> Result<bindings::TextBlock> {
    let tb = bindings::TextBlock::new()?;
    tb.SetText(s)?;
    Ok(tb)
}

/// A button's content row, decomposed into the pieces its ornament props own.
///
/// The row is a flat horizontal `StackPanel` holding, in visual order, the
/// badge (on whichever side it was asked for), the leading `SymbolIcon`, and
/// the label. Decomposing and rebuilding — rather than patching a child by
/// index, which is what `Content` and `Icon` used to do — is what lets the
/// three props be applied in any order, any number of times, without one
/// clobbering another and without the row nesting a panel inside itself.
pub(super) struct ButtonRow {
    pub icon: Option<bindings::UIElement>,
    pub badge: Option<bindings::UIElement>,
    /// Everything that is neither, in order: the label.
    pub rest: Vec<bindings::UIElement>,
    pub badge_leading: bool,
}

/// Read the row apart, **detaching** its children from the panel that held
/// them. Detaching is not optional: XAML refuses to parent an element that
/// still belongs to another, so the pieces must be free before [`write_row`]
/// can put them back in a new order.
pub(super) fn take_row(cc: &bindings::IContentControl) -> ButtonRow {
    let mut row = ButtonRow {
        icon: None,
        badge: None,
        rest: Vec::new(),
        badge_leading: false,
    };
    let Ok(content) = cc.Content() else { return row };
    let sort = |row: &mut ButtonRow, ui: bindings::UIElement| {
        if ui.cast::<bindings::ISymbolIcon>().is_ok() {
            row.icon = Some(ui);
        } else if ui.cast::<bindings::IInfoBadge>().is_ok() {
            // Leading exactly when nothing else has been seen yet.
            row.badge_leading = row.rest.is_empty() && row.icon.is_none();
            row.badge = Some(ui);
        } else if ui
            .cast::<bindings::ITextBlock>()
            .ok()
            .and_then(|tb| tb.Text().ok())
            .is_some_and(|t| t.is_empty())
        {
            // An empty label is not content. Keeping it would leave a phantom
            // gap in the row's spacing on an icon-only button.
        } else {
            row.rest.push(ui);
        }
    };
    if let Ok(panel) = content.cast::<bindings::IPanel>() {
        if let Ok(children) = panel.Children() {
            for i in 0..children.Size().unwrap_or(0) {
                if let Ok(ui) = children.GetAt(i) {
                    sort(&mut row, ui);
                }
            }
            let _ = children.Clear();
        }
    } else {
        if let Ok(ui) = content.cast::<bindings::UIElement>() {
            sort(&mut row, ui);
        }
        let _ = cc.SetContent(None::<&windows_core::IInspectable>);
    }
    row
}

/// Rebuild the row from its pieces. A single piece needs no panel — a bare
/// `SymbolIcon` content is what an icon-only button has always been.
pub(super) fn write_row(cc: &bindings::IContentControl, row: &ButtonRow) -> Result<()> {
    let mut order: Vec<&bindings::UIElement> = Vec::new();
    if row.badge_leading && let Some(b) = &row.badge {
        order.push(b);
    }
    if let Some(i) = &row.icon {
        order.push(i);
    }
    order.extend(row.rest.iter());
    if !row.badge_leading && let Some(b) = &row.badge {
        order.push(b);
    }

    match order.len() {
        0 => Ok(()),
        1 => cc.SetContent(order[0]),
        _ => {
            let panel = bindings::StackPanel::new()?;
            panel.SetOrientation(Orientation::Horizontal)?;
            panel.SetSpacing(8.0)?;
            let children = panel.cast::<bindings::IPanel>()?.Children()?;
            for ui in order {
                children.Append(ui)?;
            }
            cc.SetContent(&panel)
        }
    }
}

pub(super) fn build_nav_view_item(item: &NavViewItem) -> Result<windows_core::IInspectable> {
    if item.is_header {
        let h = bindings::NavigationViewItemHeader::new()?;
        let tb = string_as_textblock(&item.content)?;
        h.cast::<bindings::IContentControl>()?.SetContent(&tb)?;
        return h.cast();
    }
    let nv_item = bindings::NavigationViewItem::new()?;
    let tb = string_as_textblock(&item.content)?;
    nv_item
        .cast::<bindings::IContentControl>()?
        .SetContent(&tb)?;
    let tag = item.tag.clone().unwrap_or_else(|| item.content.clone());
    let tag_inspectable = windows_reference::IReference::from(tag.as_str());
    nv_item
        .cast::<bindings::IFrameworkElement>()?
        .SetTag(&tag_inspectable)?;
    if let Some(sym) = &item.icon {
        let icon_elem = bindings::SymbolIcon::CreateInstanceWithSymbol(*sym)?;
        nv_item.SetIcon(&icon_elem)?;
    }
    if !item.children.is_empty() {
        let menu = nv_item
            .cast::<bindings::INavigationViewItem2>()?
            .MenuItems()?;
        for child in &item.children {
            let child_obj = build_nav_view_item(child)?;
            menu.Append(&child_obj)?;
        }
    }
    nv_item.cast()
}

fn nav_item_tag(item: &bindings::NavigationViewItem) -> Option<String> {
    item.cast::<bindings::IFrameworkElement>()
        .ok()?
        .Tag()
        .ok()?
        .cast::<windows_reference::IReference<windows_core::HSTRING>>()
        .ok()?
        .Value()
        .ok()
        .map(|s| s.to_string_lossy())
}

pub(super) fn select_nav_item_by_tag(nv: &bindings::NavigationView, tag: &str) -> Result<()> {
    let menu = nv.MenuItems()?;

    for obj in &menu {
        let Ok(item) = obj.cast::<bindings::NavigationViewItem>() else {
            continue;
        };
        if nav_item_tag(&item).as_deref() == Some(tag) {
            let inspectable: windows_core::IInspectable = item.cast()?;
            return nv.SetSelectedItem(&inspectable);
        }
        if let Ok(children) = item.cast::<bindings::INavigationViewItem2>()?.MenuItems() {
            for child_obj in &children {
                let Ok(child) = child_obj.cast::<bindings::NavigationViewItem>() else {
                    continue;
                };
                if nav_item_tag(&child).as_deref() == Some(tag) {
                    let inspectable: windows_core::IInspectable = child.cast()?;
                    return nv.SetSelectedItem(&inspectable);
                }
            }
        }
    }
    Ok(())
}

/// Build a `MenuFlyoutItemBase` from a [`MenuItemDef`].
pub(super) fn build_menu_flyout_item_base(
    def: &MenuItemDef,
) -> Result<bindings::MenuFlyoutItemBase> {
    match def {
        MenuItemDef::Item {
            text,
            icon,
            danger,
            enabled,
            shortcut,
            // NOT SUPPORTED on this backend, and deliberately not faked: a
            // checkable row is a `ToggleMenuFlyoutItem`, a different XAML type
            // that is absent from the generated bindings. Drawing a checkmark
            // SymbolIcon on a plain `MenuFlyoutItem` would look right and still
            // report no check state to UI Automation, which is the half of the
            // feature that matters. The dcomp backend implements it fully; this
            // one renders a checkable row as an ordinary command until the
            // binding is generated.
            checked: _,
        } => {
            let item = bindings::MenuFlyoutItem::new()?;
            item.SetText(text)?;
            if let Some(sym) = icon {
                let icon_elem = bindings::SymbolIcon::CreateInstanceWithSymbol(*sym)?;
                let icon_elem: bindings::IconElement = icon_elem.cast()?;
                item.SetIcon(&icon_elem)?;
            }
            if let Some(s) = shortcut {
                item.SetKeyboardAcceleratorTextOverride(s)?;
            }
            // `IsEnabled` / `Foreground` live on `Control`.
            if !*enabled || *danger {
                let ctl: bindings::IControl = item.cast()?;
                if !*enabled {
                    ctl.SetIsEnabled(false)?;
                }
                if *danger {
                    // Destructive actions: error-red text (matches the WinUI
                    // SystemFillColorCritical accent used for danger affordances).
                    let brush = solid_brush(Color::rgb(196, 43, 28))?;
                    ctl.SetForeground(&brush)?;
                }
            }
            item.cast()
        }
        MenuItemDef::Separator => {
            let sep = bindings::MenuFlyoutSeparator::new()?;
            sep.cast()
        }
        MenuItemDef::SubItem { text, children } => {
            let sub = bindings::MenuFlyoutSubItem::new()?;
            sub.SetText(text)?;
            let sub_items = sub.Items()?;
            for child in children {
                let child_item = build_menu_flyout_item_base(child)?;
                sub_items.Append(&child_item)?;
            }
            sub.cast()
        }
    }
}

/// Recursively build a `TreeViewNode` from a [`TreeNodeDef`].
pub(super) fn build_tree_view_node(def: &TreeNodeDef) -> Result<bindings::TreeViewNode> {
    let node = bindings::TreeViewNode::new()?;
    let content: windows_core::IInspectable =
        windows_reference::IReference::<windows_core::HSTRING>::from(windows_core::HSTRING::from(
            &def.text,
        ))
        .cast()?;
    node.SetContent(&content)?;
    node.SetIsExpanded(def.is_expanded)?;
    if !def.children.is_empty() {
        let children = node.Children()?;
        for child_def in &def.children {
            let child_node = build_tree_view_node(child_def)?;
            children.Append(&child_node)?;
        }
    }
    Ok(node)
}

/// Builds a WinUI `ICommandBarElement` from a [`CommandBarCommandDef`].
pub(super) fn build_command_bar_element(
    def: &CommandBarCommandDef,
) -> Result<bindings::ICommandBarElement> {
    match def {
        CommandBarCommandDef::Button { label, icon } => {
            let btn = bindings::AppBarButton::new()?;
            btn.SetLabel(label)?;
            if let Some(sym) = icon {
                let icon_elem = bindings::SymbolIcon::CreateInstanceWithSymbol(*sym)?;
                btn.SetIcon(&icon_elem)?;
            }
            btn.cast()
        }
        CommandBarCommandDef::Toggle { label, icon } => {
            let btn = bindings::AppBarToggleButton::new()?;
            btn.SetLabel(label)?;
            if let Some(sym) = icon {
                let icon_elem = bindings::SymbolIcon::CreateInstanceWithSymbol(*sym)?;
                btn.SetIcon(&icon_elem)?;
            }
            btn.cast()
        }
        CommandBarCommandDef::Separator => {
            let sep = bindings::AppBarSeparator::new()?;
            sep.cast()
        }
    }
}
