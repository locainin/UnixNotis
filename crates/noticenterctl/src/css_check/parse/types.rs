pub(in super::super) struct CssBlock {
    // Selector text before normalization
    pub(in super::super) selector: String,
    // Raw block body without the outer braces
    pub(in super::super) block: String,
    // Cursor position for the next scan step
    pub(in super::super) next: usize,
    // Absolute offsets let lint map warnings back to line and column
    pub(in super::super) selector_start: usize,
    pub(in super::super) block_start: usize,
}

pub(in super::super) struct CssDeclaration {
    // The property name is kept as text because later lint checks match by name
    pub(in super::super) name: String,
    pub(in super::super) value: String,
    // Declaration offsets are relative to the block slice
    pub(in super::super) start: usize,
}
