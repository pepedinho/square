use crate::pane::{Direction, Pane};


pub enum Node {
    #[default]
    Placeholder,
    Leaf(Pane),
    Split {
        direction: Direction,
        ratio: u32,
        first: Box<Node>,
        second: Box<Node>
    }
}
