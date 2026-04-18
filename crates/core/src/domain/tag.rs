use super::ids::TagId;

/// Domain-level color palette. Independent of ratatui/terminal encoding —
/// widgets translate to their own color types at the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
    Custom([u8; 3]),
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    pub color: Color,
}
