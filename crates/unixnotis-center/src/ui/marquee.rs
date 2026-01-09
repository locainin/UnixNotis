//! Marquee label support for long text in constrained layouts.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Align, Overflow};

const MARQUEE_SPEED_CHARS_PER_SEC: f64 = 8.0;
const MARQUEE_PAUSE_MS: i64 = 900;

#[derive(Default)]
struct MarqueeState {
    offset: f64,
    last_time: i64,
    hold_until: i64,
    reset_pending: bool,
    enabled: bool,
    is_ticking: bool,
    is_mapped: bool,
    char_limit: usize,
    buffer: Vec<char>,
    last_rendered_offset: usize,
    full_text: String,
    render_buf: String,
}

/// Simple marquee label that slides overflow text left and resets.
#[derive(Clone)]
pub struct MarqueeLabel {
    root: gtk::Fixed,
    label: gtk::Label,
    state: Rc<RefCell<MarqueeState>>,
}

impl MarqueeLabel {
    pub fn new(css_class: &str, max_width: i32, char_limit: usize) -> Self {
        let root = gtk::Fixed::new();
        root.set_size_request(max_width, -1);
        root.set_halign(Align::Fill);
        root.set_valign(Align::Center);
        root.set_overflow(Overflow::Hidden);
        root.add_css_class("unixnotis-marquee");

        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_halign(Align::Start);
        label.set_valign(Align::Center);
        label.set_single_line_mode(true);
        label.set_wrap(false);
        label.set_hexpand(false);
        if char_limit > 0 {
            // Fixed character width avoids layout jitter as the slice changes.
            label.set_width_chars(char_limit as i32);
            label.set_max_width_chars(char_limit as i32);
        }
        label.add_css_class(css_class);

        root.put(&label, 0.0, 0.0);

        let state = Rc::new(RefCell::new(MarqueeState {
            reset_pending: true,
            enabled: false,
            is_mapped: root.is_mapped(),
            char_limit,
            last_rendered_offset: usize::MAX,
            render_buf: String::new(),
            ..Default::default()
        }));

        let instance = Self { root, label, state };
        let mapped_label = instance.clone();
        let mapped_root = mapped_label.root.clone();
        mapped_root.connect_map(move |_| {
            let mut state = mapped_label.state.borrow_mut();
            state.is_mapped = true;
            let should_start = state.enabled && !state.is_ticking;
            drop(state);
            if should_start {
                mapped_label.start_ticking();
            }
        });
        let unmapped_label = instance.clone();
        let unmapped_root = unmapped_label.root.clone();
        unmapped_root.connect_unmap(move |_| {
            let mut state = unmapped_label.state.borrow_mut();
            state.is_mapped = false;
        });

        instance
    }

    pub fn widget(&self) -> gtk::Fixed {
        self.root.clone()
    }

    pub fn set_text(&self, text: &str) {
        self.label.set_text(text);
        let mut state = self.state.borrow_mut();
        let char_limit = state.char_limit;
        state.enabled = char_limit > 0 && text.chars().count() > char_limit;
        state.reset_pending = true;
        state.offset = 0.0;
        state.hold_until = 0;
        state.last_time = 0;
        state.full_text = text.to_string();
        state.buffer = if state.enabled {
            let padded = format!("{text}   ");
            padded.chars().collect()
        } else {
            Vec::new()
        };
        state.last_rendered_offset = usize::MAX;

        let enabled = state.enabled;
        let mapped = state.is_mapped;
        let ticking = state.is_ticking;

        if enabled {
            render_visible(&mut state, 0);
            self.label.set_text(&state.render_buf);
        }
        drop(state);

        if enabled && mapped && !ticking {
            self.start_ticking();
        }
    }

    pub fn update_limits(&self, max_width: i32, char_limit: usize) {
        self.root.set_size_request(max_width, -1);
        self.label.set_width_chars(char_limit as i32);
        self.label.set_max_width_chars(char_limit as i32);
        let mut state = self.state.borrow_mut();
        state.char_limit = char_limit;
        let full_text = state.full_text.clone();
        drop(state);
        self.set_text(&full_text);
    }

    fn start_ticking(&self) {
        {
            let mut state = self.state.borrow_mut();
            if state.is_ticking {
                return;
            }
            state.is_ticking = true;
        }

        let state_tick = self.state.clone();
        let label_tick = self.label.clone();
        self.root.add_tick_callback(move |_, frame_clock| {
            let mut state = state_tick.borrow_mut();

            if !state.enabled || !state.is_mapped {
                state.is_ticking = false;
                return glib::ControlFlow::Break;
            }

            let time = frame_clock.frame_time();
            if state.last_time == 0 {
                state.last_time = time;
            }
            let delta_sec = (time - state.last_time) as f64 / 1_000_000.0;
            state.last_time = time;

            if state.reset_pending {
                state.offset = 0.0;
                state.hold_until = time + MARQUEE_PAUSE_MS * 1000;
                state.reset_pending = false;
            }

            let buffer_len = state.buffer.len();
            if buffer_len == 0 {
                return glib::ControlFlow::Continue;
            }

            if time >= state.hold_until {
                state.offset += MARQUEE_SPEED_CHARS_PER_SEC * delta_sec;
                if state.offset >= buffer_len as f64 {
                    state.offset = 0.0;
                    state.hold_until = time + MARQUEE_PAUSE_MS * 1000;
                }
            }

            let offset = state.offset.floor() as usize;
            if offset != state.last_rendered_offset {
                render_visible(&mut state, offset);
                label_tick.set_text(&state.render_buf);
                state.last_rendered_offset = offset;
            }
            glib::ControlFlow::Continue
        });
    }
}

fn render_visible(state: &mut MarqueeState, offset: usize) {
    state.render_buf.clear();
    let limit = state.char_limit;
    if limit == 0 || state.buffer.is_empty() {
        return;
    }
    state.render_buf.reserve(limit);
    let len = state.buffer.len();
    for index in 0..limit {
        let pos = (offset + index) % len;
        state.render_buf.push(state.buffer[pos]);
    }
}
