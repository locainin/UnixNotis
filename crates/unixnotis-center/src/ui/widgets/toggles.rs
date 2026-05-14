//! Toggle widgets and state synchronization logic
//!
//! This module owns toggle widget construction and interaction wiring
//! Heavy helper logic is split into focused submodules for maintainability

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::Align;
use tracing::warn;
use unixnotis_core::{css::hooks, PanelDebugLevel, ToggleLayout, ToggleWidgetConfig};

use super::utils::{run_action_command_with_completion, start_command_watch, CommandWatch};
use crate::debug;

mod css;
mod icons;
mod state;

use self::css::toggle_kind_css_class;
use self::icons::resolve_toggle_icon_name;
use self::state::{refresh_toggle_state, schedule_toggle_refresh_with_retry, ToggleRefreshGate};

pub struct ToggleGrid {
    // FlowBox root is exposed to the panel layout
    root: gtk::FlowBox,
    // Item list is kept for refresh and watch lifecycle control
    items: Vec<ToggleItem>,
}

struct ToggleItem {
    // Raw config is retained for watch/state command reuse
    config: ToggleWidgetConfig,
    // Toggle button is the interactive control rendered in the grid
    button: gtk::ToggleButton,
    // Guard blocks signal recursion when state updates set the button programmatically
    guard: Rc<Cell<bool>>,
    // Monotonic generation token drops stale async refresh completions
    refresh_gen: Rc<Cell<u64>>,
    // Local gate keeps poll and watch bursts bounded
    refresh_gate: ToggleRefreshGate,
    // Optional long-lived watch command handle for event-driven refresh paths
    watch_handle: Rc<RefCell<Option<CommandWatch>>>,
}

impl ToggleGrid {
    pub fn new(
        configs: &[ToggleWidgetConfig],
        show_tooltips: bool,
        layout: ToggleLayout,
        columns: usize,
    ) -> Option<Self> {
        // Keep only enabled entries so UI wiring stays small and deterministic
        let mut items = Vec::new();
        for config in configs {
            if !config.enabled {
                continue;
            }
            items.push(ToggleItem::new(config.clone(), show_tooltips, layout));
        }
        if items.is_empty() {
            // Caller skips widget wiring entirely when no toggles are enabled
            return None;
        }

        // FlowBox keeps toggle cards in a stable responsive row layout
        let root = gtk::FlowBox::new();
        // Class hook drives card sizing and spacing from theme css
        root.add_css_class(hooks::toggle_card::GRID);
        root.set_selection_mode(gtk::SelectionMode::None);
        let columns = flowbox_columns(columns);
        root.set_max_children_per_line(columns);
        root.set_min_children_per_line(columns);
        root.set_row_spacing(8);
        root.set_column_spacing(8);
        root.set_halign(Align::Fill);
        root.set_hexpand(true);

        for item in &items {
            // Insert in config order so visual layout remains predictable
            root.insert(&item.button, -1);
        }

        Some(Self { root, items })
    }

    pub fn root(&self) -> &gtk::FlowBox {
        // Root widget is embedded directly in the center layout
        &self.root
    }

    pub fn refresh(&self) {
        // Poll only items that are not currently watch-driven
        for item in &self.items {
            if item.needs_polling() {
                item.refresh();
            }
        }
    }

    pub fn needs_polling(&self) -> bool {
        // Shared scheduler uses this to decide whether periodic refresh is needed
        self.items.iter().any(|item| item.needs_polling())
    }

    pub fn set_watch_active(&self, active: bool) {
        // Panel visibility can enable or disable watches for all items in one pass
        for item in &self.items {
            item.set_watch_active(active);
        }
    }
}

fn flowbox_columns(columns: usize) -> u32 {
    u32::try_from(columns.max(1)).unwrap_or(u32::MAX)
}

fn toggle_action_command<'a>(
    toggle_cmd: Option<&'a String>,
    on_cmd: Option<&'a String>,
    off_cmd: Option<&'a String>,
    active: bool,
) -> Option<&'a String> {
    toggle_cmd.or(if active { on_cmd } else { off_cmd })
}

fn should_reset_after_action(toggle_cmd: Option<&String>, state_cmd: Option<&String>) -> bool {
    // Without a state command, the card cannot know whether the action changed system state
    toggle_cmd.is_some() && state_cmd.is_none()
}

fn reset_toggle_visual_state(button: &gtk::ToggleButton, guard: &Rc<Cell<bool>>) {
    if button.is_active() {
        // Guard blocks the reset from dispatching the same action again
        guard.set(true);
        button.set_active(false);
        guard.set(false);
    }
    if button.has_css_class(hooks::shared_state::ACTIVE) {
        button.remove_css_class(hooks::shared_state::ACTIVE);
    }
}

impl ToggleItem {
    fn new(config: ToggleWidgetConfig, show_tooltips: bool, layout: ToggleLayout) -> Self {
        // Guard and generation tokens are per-item to isolate async updates
        let guard = Rc::new(Cell::new(false));
        // Refresh generation only lives on the GTK main loop
        // Rc<Cell<_>> avoids atomic refcount and atomic integer traffic here
        let refresh_gen = Rc::new(Cell::new(0_u64));
        let refresh_gate = ToggleRefreshGate::new();

        // Build base toggle card
        let button = gtk::ToggleButton::new();
        // Base class applies shared visual treatment for all toggle cards
        button.add_css_class(hooks::toggle_card::ROOT);
        button.add_css_class(hooks::toggle_card::HAS_ICON);
        button.set_focusable(true);
        // Tooltip stays optional so hover can stay visually quiet in compact layouts
        if show_tooltips {
            button.set_tooltip_text(Some(&config.label));
        }

        // Stable per-kind CSS classes let themes target each toggle consistently
        if let Some(kind) = config.kind.as_deref() {
            if let Some(class) = toggle_kind_css_class(kind) {
                // Kind class allows targeted color accents and hover behavior per toggle
                button.add_css_class(&class);
            }
        }

        let content_orientation = match layout {
            ToggleLayout::Horizontal => gtk::Orientation::Horizontal,
            ToggleLayout::Vertical => gtk::Orientation::Vertical,
        };
        let content = gtk::Box::new(content_orientation, 8);
        // Centered content keeps icon and label aligned across theme variants
        content.set_halign(Align::Center);
        content.set_valign(Align::Center);
        content.add_css_class(hooks::toggle_card::CONTENT);

        // Resolve themed icon names before creating the image so fallback is explicit
        let icon_name = resolve_toggle_icon_name(&config);
        if icon_name != config.icon {
            warn!(
                requested = %config.icon,
                resolved = %icon_name,
                label = %config.label,
                "toggle icon missing in theme; using fallback"
            );
        }
        let icon = gtk::Image::from_icon_name(&icon_name);
        // Icon class controls size and tint in one place
        icon.add_css_class(hooks::toggle_card::ICON);

        let label = gtk::Label::new(Some(&config.label));
        // Label class controls typography and spacing with icon
        label.add_css_class(hooks::toggle_card::LABEL);
        label.set_xalign(match layout {
            ToggleLayout::Horizontal => 0.0,
            ToggleLayout::Vertical => 0.5,
        });
        label.set_justify(match layout {
            ToggleLayout::Horizontal => gtk::Justification::Left,
            ToggleLayout::Vertical => gtk::Justification::Center,
        });
        label.set_wrap(false);

        content.append(&icon);
        content.append(&label);
        button.set_child(Some(&content));

        // Clone command fields once so toggle callback stays allocation-light
        let guard_clone = guard.clone();
        let state_cmd = config.state_cmd.clone();
        let toggle_cmd = config.toggle_cmd.clone();
        let on_cmd = config.on_cmd.clone();
        let off_cmd = config.off_cmd.clone();
        let refresh_gen_for_toggle = refresh_gen.clone();
        let label = config.label.clone();

        button.connect_toggled(move |button| {
            // Programmatic updates should not retrigger command execution
            if guard_clone.get() {
                return;
            }

            debug::log(PanelDebugLevel::Info, || {
                format!("toggle '{}' set to {}", label, button.is_active())
            });

            // Single-command toggles let custom buttons run one user-defined action
            // instead of forcing every button into separate on/off commands
            let command = toggle_action_command(
                toggle_cmd.as_ref(),
                on_cmd.as_ref(),
                off_cmd.as_ref(),
                button.is_active(),
            );
            // Expected tracks the immediate UI state chosen by the user
            let expected = button.is_active();
            let guard = guard_clone.clone();
            let refresh_gen = refresh_gen_for_toggle.clone();
            let button = button.clone();
            let reset_after_action =
                should_reset_after_action(toggle_cmd.as_ref(), state_cmd.as_ref());

            if let Some(cmd) = command.cloned() {
                let state_cmd_for_retry = state_cmd.clone();
                let label_for_retry = label.clone();
                // Action completion is the clean handoff into retry-based reconciliation
                run_action_command_with_completion(cmd, "toggle action", move |failed| {
                    if failed {
                        debug::log(PanelDebugLevel::Warn, || {
                            format!(
                                "toggle action failed; reconciling '{}' back to real state",
                                label_for_retry
                            )
                        });
                    }

                    if let Some(state_cmd) = state_cmd_for_retry.clone() {
                        schedule_toggle_refresh_with_retry(
                            state_cmd,
                            expected,
                            button.clone(),
                            guard.clone(),
                            refresh_gen.clone(),
                        );
                    } else if reset_after_action {
                        // Stateless custom actions should not leave the card visually checked
                        reset_toggle_visual_state(&button, &guard);
                    }
                });
            } else if let Some(state_cmd) = state_cmd.clone() {
                // Command-free toggles still use the same reconcile path
                schedule_toggle_refresh_with_retry(state_cmd, expected, button, guard, refresh_gen);
            } else {
                // No command and no state is inert, so undo the visual edge immediately
                reset_toggle_visual_state(&button, &guard);
            }
        });

        let item = Self {
            config,
            button,
            guard,
            refresh_gen,
            refresh_gate,
            watch_handle: Rc::new(RefCell::new(None)),
        };
        // Prime initial state once after widget construction
        item.refresh();
        item
    }

    fn refresh(&self) {
        // Items without state commands are display-only and skip refresh work
        if let Some(state_cmd) = self.config.state_cmd.as_ref() {
            refresh_toggle_state(
                state_cmd,
                &self.button,
                &self.guard,
                &self.refresh_gen,
                &self.refresh_gate,
            );
        }
    }

    fn needs_polling(&self) -> bool {
        let mut handle = self.watch_handle.borrow_mut();
        if let Some(watch) = handle.as_ref() {
            // Drop inactive watch handles so polling can backfill state updates
            if !watch.is_active() {
                handle.take();
                return true;
            }
            return false;
        }
        true
    }

    fn set_watch_active(&self, active: bool) {
        // Watch lifecycle is skipped when required commands are absent
        if self.config.watch_cmd.is_none() || self.config.state_cmd.is_none() {
            return;
        }

        let mut handle = self.watch_handle.borrow_mut();
        if active {
            if handle.is_none() {
                debug::log(PanelDebugLevel::Info, || {
                    format!("toggle watch enabled: {}", self.config.label)
                });
                *handle = self.start_watch();
            }
        } else {
            if handle.is_some() {
                debug::log(PanelDebugLevel::Info, || {
                    format!("toggle watch disabled: {}", self.config.label)
                });
            }
            // Dropping the handle stops the background watcher
            handle.take();
        }
    }

    fn start_watch(&self) -> Option<CommandWatch> {
        // Watch mode requires both watch and state commands
        let watch_cmd = self.config.watch_cmd.as_ref()?;
        let state_cmd = self.config.state_cmd.as_ref()?.clone();
        let button = self.button.clone();
        let guard = self.guard.clone();
        let refresh_gen = self.refresh_gen.clone();
        let refresh_gate = self.refresh_gate.clone();

        // Watch callbacks trigger the same refresh path as polling so semantics stay identical
        start_command_watch(watch_cmd, move || {
            refresh_toggle_state(&state_cmd, &button, &guard, &refresh_gen, &refresh_gate);
        })
    }
}

#[cfg(test)]
#[path = "tests/toggles.rs"]
mod tests;
