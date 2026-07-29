#![allow(clippy::upper_case_acronyms)]
// SUBTRACE FORK PATCH (rdev 0.5.3): key names come from
// `CGEventKeyboardGetUnicodeString` instead of TIS + `UCKeyTranslate`. The TIS
// functions (`TISCopyCurrentKeyboardInputSource` / `TISGetInputSourceProperty`)
// contain `dispatch_assert_queue(main)` on macOS Sequoia and abort the process
// when called off the main thread. rdev cannot guarantee it is on it: the
// event-tap callback runs on whichever thread installed the tap, and
// `KeyboardState` is `Send`. Trade-off: dead-key sequences
// ("´" + "e" → "é") are no longer composed across events, since the synthetic
// event carries no dead-key state. Shift/caps reach the translation as event
// flags; `examples/subtrace_offmain_layout.rs` asserts that they do.
use crate::macos::keycodes::code_from_key;
use crate::rdev::{EventType, Key, KeyboardState};
use core_foundation::string::UniChar;
use core_graphics::event::{CGEvent, CGEventFlags};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use foreign_types::ForeignType;
use std::convert::TryInto;

type UniCharCount = usize;

const BUF_LEN: usize = 8;

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventKeyboardGetUnicodeString(
        event: core_graphics::sys::CGEventRef,
        max_string_length: UniCharCount,
        actual_string_length: *mut UniCharCount,
        unicode_string: *mut UniChar,
    );
}

pub struct Keyboard {
    shift: bool,
    caps_lock: bool,
}

impl Keyboard {
    pub fn new() -> Option<Keyboard> {
        Some(Keyboard {
            shift: false,
            caps_lock: false,
        })
    }

    /// Reads the Unicode string the window server already stored on `cg_event`.
    /// Touches no input-source state, so it is safe on any thread.
    pub(crate) unsafe fn string_from_event(cg_event: &CGEvent) -> Option<String> {
        let mut buff = [0 as UniChar; BUF_LEN];
        let mut length: UniCharCount = 0;
        CGEventKeyboardGetUnicodeString(
            cg_event.as_ptr(),
            BUF_LEN as UniCharCount,
            &mut length as *mut UniCharCount,
            buff.as_mut_ptr(),
        );
        let length = length.min(BUF_LEN);
        if length == 0 {
            return None;
        }
        String::from_utf16(&buff[..length]).ok()
    }

    pub(crate) unsafe fn create_string_for_key(
        &mut self,
        code: u32,
        flags: CGEventFlags,
    ) -> Option<String> {
        let code: u16 = code.try_into().ok()?;
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
        let event = CGEvent::new_keyboard_event(source, code, true).ok()?;
        event.set_flags(flags);
        Self::string_from_event(&event)
    }
}

impl KeyboardState for Keyboard {
    fn add(&mut self, event_type: &EventType) -> Option<String> {
        match event_type {
            EventType::KeyPress(key) => match key {
                Key::ShiftLeft | Key::ShiftRight => {
                    self.shift = true;
                    None
                }
                Key::CapsLock => {
                    self.caps_lock = !self.caps_lock;
                    None
                }
                key => {
                    let code = code_from_key(*key)?;
                    let mut flags = CGEventFlags::CGEventFlagNull;
                    if self.shift {
                        flags |= CGEventFlags::CGEventFlagShift;
                    }
                    if self.caps_lock {
                        flags |= CGEventFlags::CGEventFlagAlphaShift;
                    }
                    unsafe { self.create_string_for_key(code.into(), flags) }
                }
            },
            EventType::KeyRelease(key) => match key {
                Key::ShiftLeft | Key::ShiftRight => {
                    self.shift = false;
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.shift = false;
        self.caps_lock = false;
    }
}
