//! Which cells are selected, by the explorer's rules.
//!
//! A click selects one cell and a click with the platform modifier toggles
//! one. A shift click selects the run from the anchor, the cell last clicked,
//! and adds it to a selection of several, as Finder does. Arrow keys move a
//! single selection. Cells are grid indices, so a selection belongs to the
//! folder it was made in and is dropped with it.

use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Selection {
	selected: BTreeSet<u32>,
	/// Where a shift click's run starts.
	anchor: Option<u32>,
	/// The keyboard cursor, drawn even when its cell is not selected.
	focus: Option<u32>,
}

impl Selection {
	/// A click on `index`. `toggle` is the platform modifier, `extend` is
	/// shift. Shift with nothing clicked yet selects the one cell.
	pub fn click(&mut self, index: u32, toggle: bool, extend: bool) {
		match self.anchor {
			Some(anchor) if extend => {
				if self.selected.len() <= 1 {
					self.selected.clear();
				}
				self.selected.extend(anchor.min(index)..=anchor.max(index));
			}
			_ if toggle => {
				if !self.selected.remove(&index) {
					self.selected.insert(index);
				}
			}
			_ => {
				self.selected.clear();
				self.selected.insert(index);
			}
		}
		self.anchor = Some(index);
		self.focus = Some(index);
	}

	/// Select `index` alone, as an arrow key does.
	pub fn select_only(&mut self, index: u32) {
		self.selected.clear();
		self.selected.insert(index);
		self.anchor = Some(index);
		self.focus = Some(index);
	}

	/// Select every one of `len` cells.
	pub fn select_all(&mut self, len: u32) {
		self.selected = (0..len).collect();
	}

	pub fn clear(&mut self) {
		*self = Selection::default();
	}

	pub fn contains(&self, index: u32) -> bool {
		self.selected.contains(&index)
	}

	pub fn len(&self) -> usize {
		self.selected.len()
	}

	pub fn is_empty(&self) -> bool {
		self.selected.is_empty()
	}

	/// Selected cells, in grid order.
	pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
		self.selected.iter().copied()
	}

	pub fn focus(&self) -> Option<u32> {
		self.focus
	}
}

/// Arrow-key directions over the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
	Left,
	Right,
	Up,
	Down,
}

/// The cell an arrow key lands on from `from`, in `len` cells laid out
/// `cols` to a row. Left and right step through reading order. Up and down
/// keep the column, and down onto a short last row lands on its last cell.
/// At an edge the cursor stays where it is.
pub fn step(from: u32, direction: Direction, cols: u32, len: u32) -> u32 {
	let last = len.saturating_sub(1);
	let cols = cols.max(1);
	match direction {
		Direction::Left => from.saturating_sub(1),
		Direction::Right => (from + 1).min(last),
		Direction::Up if from >= cols => from - cols,
		Direction::Down if from / cols < last / cols => (from + cols).min(last),
		Direction::Up | Direction::Down => from,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn selected(selection: &Selection) -> Vec<u32> {
		selection.iter().collect()
	}

	#[test]
	fn a_click_selects_one_cell() {
		let mut selection = Selection::default();
		selection.click(4, false, false);
		selection.click(7, false, false);
		assert_eq!(selected(&selection), [7]);
		assert_eq!(selection.focus(), Some(7));
	}

	#[test]
	fn a_modifier_click_toggles_a_cell() {
		let mut selection = Selection::default();
		selection.click(1, false, false);
		selection.click(3, true, false);
		assert_eq!(selected(&selection), [1, 3]);
		selection.click(1, true, false);
		assert_eq!(selected(&selection), [3]);
	}

	#[test]
	fn a_shift_click_selects_the_run_from_the_anchor() {
		let mut selection = Selection::default();
		selection.click(6, false, false);
		selection.click(2, false, true);
		assert_eq!(selected(&selection), [2, 3, 4, 5, 6]);
	}

	#[test]
	fn a_shift_click_adds_its_run_to_a_selection_of_several() {
		let mut selection = Selection::default();
		selection.click(0, false, false);
		selection.click(10, true, false);
		selection.click(12, false, true);
		assert_eq!(selected(&selection), [0, 10, 11, 12]);

		// The anchor moved to the shift-clicked cell, so the next run starts
		// there.
		selection.click(14, false, true);
		assert_eq!(selected(&selection), [0, 10, 11, 12, 13, 14]);
	}

	#[test]
	fn a_shift_click_with_nothing_clicked_selects_one_cell() {
		let mut selection = Selection::default();
		selection.click(5, false, true);
		assert_eq!(selected(&selection), [5]);
	}

	#[test]
	fn select_all_and_clear() {
		let mut selection = Selection::default();
		selection.click(2, false, false);
		selection.select_all(4);
		assert_eq!(selected(&selection), [0, 1, 2, 3]);
		selection.clear();
		assert!(selection.is_empty());
		assert_eq!(selection.focus(), None);
	}

	#[test]
	fn arrows_step_through_the_grid_and_stop_at_its_edges() {
		// Four columns, ten cells: rows 0..4, 4..8, and a short 8..10.
		assert_eq!(step(5, Direction::Left, 4, 10), 4);
		assert_eq!(step(0, Direction::Left, 4, 10), 0);
		assert_eq!(step(9, Direction::Right, 4, 10), 9);
		assert_eq!(step(5, Direction::Up, 4, 10), 1);
		assert_eq!(step(2, Direction::Up, 4, 10), 2);
		assert_eq!(step(1, Direction::Down, 4, 10), 5);
		assert_eq!(step(7, Direction::Down, 4, 10), 9);
		assert_eq!(step(9, Direction::Down, 4, 10), 9);
	}
}
