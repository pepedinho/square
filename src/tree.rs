use ratatui::layout::Rect;
use tokio::sync::mpsc::UnboundedSender;

use crate::pane::Pane;

/// Splitting direction of [`Node::Split`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Side-by-side: divide width between the two children.
    Vertical,
    /// Stacked: divide height between the two children.
    Horizontal,
}

/// Recursive pane tree.
#[allow(clippy::large_enum_variant)]
#[derive(Default)]
pub enum Node {
    /// Transient placeholder used by  [`std::mem::take`] during split surgery.
    /// No user-facing operation ever produces it.
    #[default]
    Placeholder,
    /// A leaf owning a real [`Pane`].
    Leaf(Pane),
    /// A branch splitting its rect between two child nodes.
    Split {
        direction: Direction,
        /// Share (0..=1) of the split dimension taken by `first`.
        ratio: f32,
        first: Box<Node>,
        second: Box<Node>,
    },
}

fn split_rect(rect: Rect, direction: Direction, ratio: f32) -> (Rect, Rect) {
    let ratio = ratio.clamp(0.0, 1.0);
    match direction {
        Direction::Vertical => {
            let max_w1 = rect.width.saturating_sub(1);
            let w1 = if max_w1 == 0 {
                0
            } else {
                ((rect.width as f32 * ratio).round() as u16).clamp(1, max_w1)
            };
            (
                Rect::new(rect.x, rect.y, w1, rect.height),
                Rect::new(
                    rect.x.saturating_add(w1),
                    rect.y,
                    rect.width - w1,
                    rect.height,
                ),
            )
        }
        Direction::Horizontal => {
            let max_h1 = rect.height.saturating_sub(1);
            let h1 = if max_h1 == 0 {
                0
            } else {
                ((rect.height as f32 * ratio).round() as u16).clamp(1, max_h1)
            };
            (
                Rect::new(rect.x, rect.y, rect.width, h1),
                Rect::new(
                    rect.x,
                    rect.y.saturating_add(h1),
                    rect.width,
                    rect.height - h1,
                ),
            )
        }
    }
}

impl Node {
    /// Recompute every leaf's rect from `rect` and resize the underlying PTYs.
    /// This is what preserves nested-split structure on terminal resize.
    pub fn layout(&mut self, rect: Rect) {
        match self {
            Node::Placeholder => {}
            Node::Leaf(pane) => pane.set_rect(rect),
            Node::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (r1, r2) = split_rect(rect, *direction, *ratio);
                first.layout(r1);
                second.layout(r2);
            }
        }
    }

    /// Does this subtree contain the leaf with the given `id`?
    pub fn contains(&self, id: usize) -> bool {
        match self {
            Node::Placeholder => false,
            Node::Leaf(pane) => pane.id == id,
            Node::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    /// Find the leaf whose `id` matches, as a shared reference.
    pub fn find_leaf(&self, id: usize) -> Option<&Pane> {
        match self {
            Node::Placeholder => None,
            Node::Leaf(pane) if pane.id == id => Some(pane),
            Node::Leaf(_) => None,
            Node::Split { first, second, .. } => {
                first.find_leaf(id).or_else(|| second.find_leaf(id))
            }
        }
    }

    /// Mutable counterpart of [`find_leaf`](Self::find_leaf)
    pub fn find_leaf_mut(&mut self, id: usize) -> Option<&mut Pane> {
        match self {
            Node::Placeholder => None,
            Node::Leaf(pane) if pane.id == id => Some(pane),
            Node::Leaf(_) => None,
            Node::Split { first, second, .. } => {
                if let Some(pane) = first.find_leaf_mut(id) {
                    return Some(pane);
                }
                second.find_leaf_mut(id)
            }
        }
    }

    /// Split the leaf `id`: it keeps the first half of its rect (resized),
    /// a brand-new pane takes the second half.
    ///
    /// Return `false` if `id` is not in this tree.
    pub fn split_active(
        &mut self,
        id: usize,
        direction: Direction,
        tx: UnboundedSender<(usize, Vec<u8>)>,
        new_id: usize,
    ) -> bool {
        if !self.contains(id) {
            return false;
        }

        let taken = std::mem::take(self);
        *self = taken.into_split(id, direction, tx, new_id);
        true
    }

    fn into_split(
        self,
        id: usize,
        direction: Direction,
        tx: UnboundedSender<(usize, Vec<u8>)>,
        new_id: usize,
    ) -> Node {
        match self {
            Node::Leaf(pane) if pane.id == id => {
                let (r_kept, r_new) = split_rect(pane.rect, direction, 0.5);

                let mut kept = pane;
                kept.set_rect(r_kept);

                let mut sibling = Pane::new(new_id, tx, r_new.height, r_new.width);
                sibling.set_rect(r_new);

                Node::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(Node::Leaf(kept)),
                    second: Box::new(Node::Leaf(sibling)),
                }
            }
            Node::Leaf(pane) => Node::Leaf(pane),
            Node::Split {
                direction: d,
                ratio,
                first,
                second,
            } if first.contains(id) => Node::Split {
                direction: d,
                ratio,
                first: Box::new(first.into_split(id, direction, tx, new_id)),
                second,
            },
            Node::Split {
                direction: d,
                ratio,
                first,
                second,
            } => Node::Split {
                direction: d,
                ratio,
                first,
                second: Box::new(second.into_split(id, direction, tx, new_id)),
            },
            Node::Placeholder => Node::Placeholder,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::sync::mpsc;

    /// A tree holding a single real pane (spawns a shell + reader thread).
    fn leaf(id: usize, rows: u16, cols: u16) -> Node {
        let (tx, _rx) = mpsc::unbounded_channel();
        Node::Leaf(Pane::new(id, tx, rows, cols))
    }

    fn channel() -> UnboundedSender<(usize, Vec<u8>)> {
        let (tx, _rx) = mpsc::unbounded_channel();
        tx
    }

    fn nested_tree() -> Node {
        Node::Split {
            direction: Direction::Horizontal,
            ratio: 0.5,
            first: Box::new(Node::Split {
                direction: Direction::Vertical,
                ratio: 0.5,
                first: Box::new(leaf(1, 5, 10)),
                second: Box::new(leaf(2, 5, 10)),
            }),
            second: Box::new(leaf(3, 5, 10)),
        }
    }

    #[test]
    fn split_rect_vertical_halves_width() {
        let rect = Rect::new(2, 3, 10, 4);
        let (a, b) = split_rect(rect, Direction::Vertical, 0.5);

        assert_eq!(a, Rect::new(2, 3, 5, 4));
        assert_eq!(b, Rect::new(7, 3, 5, 4));
        assert_eq!(a.width + b.width, rect.width);
    }

    #[test]
    fn split_rect_horizontal_halves_height() {
        let rect = Rect::new(1, 1, 4, 10);
        let (a, b) = split_rect(rect, Direction::Horizontal, 0.5);

        assert_eq!(a, Rect::new(1, 1, 4, 5));
        assert_eq!(b, Rect::new(1, 6, 4, 5));
        assert_eq!(a.height + b.height, rect.height);
    }

    #[test]
    fn split_rect_clamps_ratio_between_cells() {
        let rect = Rect::new(0, 0, 10, 1);

        let (a, _) = split_rect(rect, Direction::Vertical, 0.0);
        assert_eq!(a.width, 1);

        let (a, b) = split_rect(rect, Direction::Vertical, 1.0);
        assert_eq!(a.width, 9);
        assert_eq!(b.width, 1);
    }

    #[test]
    fn split_rect_handles_single_column() {
        let rect = Rect::new(0, 0, 1, 1);
        let (a, b) = split_rect(rect, Direction::Vertical, 0.5);

        assert_eq!(a.width, 0);
        assert_eq!(b.width, 1);
    }

    #[test]
    fn layout_partitions_rect_across_nested_splits() {
        let mut root = nested_tree();
        root.layout(Rect::new(0, 0, 20, 10));

        assert_eq!(root.find_leaf(1).unwrap().rect, Rect::new(0, 0, 10, 5));
        assert_eq!(root.find_leaf(2).unwrap().rect, Rect::new(10, 0, 10, 5));
        assert_eq!(root.find_leaf(3).unwrap().rect, Rect::new(0, 5, 20, 5));

        let total = root.find_leaf(1).unwrap().rect.area()
            + root.find_leaf(2).unwrap().rect.area()
            + root.find_leaf(3).unwrap().rect.area();
        assert_eq!(total, 20 * 10);
    }

    #[test]
    fn layout_resizes_leaf_parsers() {
        let mut root = nested_tree();
        root.layout(Rect::new(0, 0, 20, 10));

        assert_eq!(root.find_leaf(1).unwrap().parser.screen().size(), (5, 10));
        assert_eq!(root.find_leaf(2).unwrap().parser.screen().size(), (5, 10));
        assert_eq!(root.find_leaf(3).unwrap().parser.screen().size(), (5, 20));
    }

    #[test]
    fn layout_is_a_noop_for_placeholder() {
        let mut root = Node::Placeholder;
        root.layout(Rect::new(0, 0, 20, 10));
        assert!(matches!(root, Node::Placeholder));
    }

    #[test]
    fn contains_and_find_report_nested_leaves() {
        let root = nested_tree();

        assert!(root.contains(1));
        assert!(root.contains(2));
        assert!(root.contains(3));
        assert!(!root.contains(99));

        assert!(root.find_leaf(1).is_some());
        assert!(root.find_leaf(2).is_some());
        assert!(root.find_leaf(3).is_some());
        assert!(root.find_leaf(99).is_none());
    }

    #[test]
    fn find_leaf_mut_mutates_the_matching_pane() {
        let mut root = nested_tree();

        let pane = root.find_leaf_mut(2).unwrap();
        pane.title = "edited".to_string();

        assert_eq!(root.find_leaf(2).unwrap().title, "edited");
        assert_eq!(root.find_leaf(1).unwrap().title, "");
    }

    #[test]
    fn split_active_turns_leaf_into_a_pair() {
        let tx = channel();
        let mut root = Node::Leaf(Pane::new(0, tx.clone(), 10, 20));
        root.layout(Rect::new(0, 0, 20, 10));

        assert!(root.split_active(0, Direction::Vertical, tx, 1));

        assert!(matches!(root, Node::Split { .. }));
        assert_eq!(root.find_leaf(0).unwrap().rect, Rect::new(0, 0, 10, 10));
        assert_eq!(root.find_leaf(1).unwrap().rect, Rect::new(10, 0, 10, 10));
        assert!(root.contains(0));
        assert!(root.contains(1));
    }

    #[test]
    fn split_active_sizes_the_new_sibling_pty() {
        let tx = channel();
        let mut root = Node::Leaf(Pane::new(0, tx.clone(), 10, 20));
        root.layout(Rect::new(0, 0, 20, 10));

        root.split_active(0, Direction::Vertical, tx, 1);

        let (kept, sibling) = (root.find_leaf(0).unwrap(), root.find_leaf(1).unwrap());
        assert_eq!(kept.parser.screen().size(), (10, 10));
        assert_eq!(sibling.parser.screen().size(), (10, 10));
    }

    #[test]
    fn split_active_on_missing_id_is_a_noop() {
        let tx = channel();
        let mut root = leaf(0, 10, 20);

        assert!(!root.split_active(42, Direction::Vertical, tx, 1));
        assert!(matches!(root, Node::Leaf(_)));
    }

    #[test]
    fn split_active_on_nested_leaf_only_touches_that_subtree() {
        let tx = channel();
        let mut root = Node::Leaf(Pane::new(0, tx.clone(), 12, 30));
        root.layout(Rect::new(0, 0, 30, 12));

        // id 0 -> vertical pair (0,1), then id 1 -> horizontal pair (1,2)
        root.split_active(0, Direction::Vertical, tx.clone(), 1);
        root.split_active(1, Direction::Horizontal, tx, 2);

        assert_eq!(root.find_leaf(0).unwrap().rect, Rect::new(0, 0, 15, 12));
        assert_eq!(root.find_leaf(1).unwrap().rect, Rect::new(15, 0, 15, 6));
        assert_eq!(root.find_leaf(2).unwrap().rect, Rect::new(15, 6, 15, 6));

        let total = root.find_leaf(0).unwrap().rect.area()
            + root.find_leaf(1).unwrap().rect.area()
            + root.find_leaf(2).unwrap().rect.area();
        assert_eq!(total, 30 * 12);
    }
}
