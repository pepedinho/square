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
