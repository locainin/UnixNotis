use clap::ValueEnum;

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum DndState {
    // Explicitly enable DND
    On,
    // Explicitly disable DND
    Off,
    // Toggle based on current daemon state
    Toggle,
}
