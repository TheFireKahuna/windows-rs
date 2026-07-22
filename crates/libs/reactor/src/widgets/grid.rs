use super::*;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Grid {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub rows: Vec<GridLength>,
    pub columns: Vec<GridLength>,
    pub row_spacing: f64,
    pub column_spacing: f64,
    /// Children carry no cell of their own and flow across the tracks — see
    /// [`Grid::auto_flow`].
    pub auto_flow: bool,
    pub children: Vec<Element>,
}

impl Widget for Grid {
    widget_header!(ControlKind::Grid);
    fn bindings(&self) -> PropBindings {
        let mut out = generated::grid_bindings(self);
        if !self.rows.is_empty() {
            out.push(Binding::Prop(
                Prop::GridRows,
                PropValue::GridLengths(self.rows.clone()),
            ));
        }
        if !self.columns.is_empty() {
            out.push(Binding::Prop(
                Prop::GridColumns,
                PropValue::GridLengths(self.columns.clone()),
            ));
        }
        if self.auto_flow {
            out.push(Binding::Prop(Prop::GridAutoFlow, PropValue::Bool(true)));
        }
        out
    }
    fn children(&self) -> Children<'_> {
        Children::Keyed(&self.children)
    }
}

impl Grid {
    pub fn rows<I: IntoIterator<Item = GridLength>>(mut self, it: I) -> Self {
        self.rows = it.into_iter().collect();
        self
    }

    pub fn columns<I: IntoIterator<Item = GridLength>>(mut self, it: I) -> Self {
        self.columns = it.into_iter().collect();
        self
    }

    /// Fill the width with as many equal columns of at least `min_width` DIPs
    /// as fit, reflowing as the grid resizes. Children need no placement —
    /// auto-placement flows them across and wraps to a new row.
    ///
    /// See [`GridLength::AutoFill`]; replaces any columns already set.
    pub fn auto_fill_columns(mut self, min_width: f64) -> Self {
        self.columns = vec![GridLength::AutoFill(min_width)];
        self
    }

    /// Let this grid place its own children: they carry no row/column, and flow
    /// across the tracks wrapping as they fill.
    ///
    /// Implied by [`auto_fill_columns`](Self::auto_fill_columns), where the track
    /// count is unknowable to the app. Ask for it explicitly when the app DOES
    /// compute the count — see [`balanced_columns`](Self::balanced_columns) — so
    /// a reflow rewrites one track list instead of every child's cell.
    ///
    /// Off by default: an unplaced child belongs to cell (0, 0), which is XAML
    /// parity and what a deliberately overlapping pair relies on.
    pub fn auto_flow(mut self) -> Self {
        self.auto_flow = true;
        self
    }

    /// As many equal columns as fit `width`, reduced until the rows come out
    /// even — so a trailing row is never a lone orphan.
    ///
    /// This one takes a MEASURED width, because balancing cannot be a track
    /// function: the balanced count is picked from a discrete set derived from
    /// the item count and the count that fits, and a track function cannot
    /// choose based on its own result. Prefer
    /// [`auto_fill_columns`](Self::auto_fill_columns), which needs no
    /// measurement at all, unless orphan rows actually matter.
    ///
    /// What it does avoid is the per-child cost: children auto-flow, so a
    /// reflow rewrites this grid's track list and nothing else. Placing each
    /// child by hand instead makes the same reflow a style write per child.
    ///
    /// `width` of `0.0` (before the first measurement) yields a single column.
    pub fn balanced_columns(mut self, min_width: f64, items: usize, width: f64) -> Self {
        let cols = balanced_column_count(min_width, items, width, self.column_spacing);
        self.columns = std::iter::repeat_n(GridLength::STAR, cols).collect();
        self.auto_flow = true;
        self
    }

    pub fn row_spacing(mut self, v: f64) -> Self {
        self.row_spacing = v;
        self
    }

    pub fn column_spacing(mut self, v: f64) -> Self {
        self.column_spacing = v;
        self
    }
}

/// How many equal, `gap`-separated columns of at least `min_width` fit `width`,
/// then reduced so `items` divide into even rows.
///
/// Separated from the builder so the rule is testable without a tree: the
/// balancing step is the part that is easy to get subtly wrong, and getting it
/// wrong shows up as a single orphan tile rather than as a failure.
#[must_use]
pub fn balanced_column_count(min_width: f64, items: usize, width: f64, gap: f64) -> usize {
    if items == 0 {
        return 1;
    }
    // Fits: n columns need n*min + (n-1)*gap, i.e. (width + gap) / (min + gap).
    let fits = ((width + gap) / (min_width + gap)).floor();
    let fits = if fits.is_finite() && fits >= 1.0 { fits as usize } else { 1 };
    let cols = fits.min(items);
    // Even rows: with that many rows, this is the widest row that stays even.
    let rows = items.div_ceil(cols);
    items.div_ceil(rows)
}

pub fn grid(children: impl IntoElements) -> Grid {
    Grid {
        children: children.into_elements(),
        ..Grid::default()
    }
}

#[cfg(test)]
mod tests {
    use super::balanced_column_count;

    /// The count must never exceed what actually fits — balancing may only
    /// REDUCE columns, never widen past the measured room.
    #[test]
    fn balancing_never_exceeds_what_fits() {
        for width in [0.0, 100.0, 320.0, 900.0, 4000.0] {
            for items in 1..24 {
                let n = balanced_column_count(200.0, items, width, 8.0);
                let fits = (((width + 8.0) / 208.0).floor().max(1.0)) as usize;
                assert!(n >= 1, "{items} items at {width}: no columns");
                assert!(n <= fits.max(1), "{items} items at {width}: {n} > {fits} that fit");
                assert!(n <= items, "{items} items: {n} columns");
            }
        }
    }

    /// The property the mode exists for: no trailing row holds fewer items than
    /// the rows above it by more than the balancing allows — concretely, every
    /// row is full except possibly the last, and the last is never a lone tile
    /// when a rounder split was available.
    #[test]
    fn balanced_rows_leave_no_orphan() {
        // 7 items in room for 4 columns balances to 4 (rows of 4+3), not 4+1+…
        assert_eq!(balanced_column_count(200.0, 7, 840.0, 8.0), 4);
        // 5 items in room for 4 columns: 2 rows, so 3 per row (3+2), not 4+1.
        assert_eq!(balanced_column_count(200.0, 5, 840.0, 8.0), 3);
        // 9 items in room for 5: 2 rows, 5 per row (5+4) — already even enough.
        assert_eq!(balanced_column_count(200.0, 9, 1050.0, 8.0), 5);
    }

    /// Degenerate inputs must still yield a usable grid rather than a division
    /// by zero or a zero-track template.
    #[test]
    fn degenerate_inputs_yield_one_column() {
        assert_eq!(balanced_column_count(200.0, 0, 900.0, 8.0), 1);
        assert_eq!(balanced_column_count(200.0, 6, 0.0, 8.0), 1);
        assert_eq!(balanced_column_count(0.0, 6, 0.0, 0.0), 1);
    }
}
