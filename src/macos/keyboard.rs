#![allow(clippy::upper_case_acronyms)]
// SUBTRACE FORK PATCH (rdev 0.5.3): key names come from
// `CGEventKeyboardGetUnicodeString` instead of TIS + `UCKeyTranslate`. The TIS
// functions (`TISCopyCurrentKeyboardInputSource` / `TISGetInputSourceProperty`)
// contain `dispatch_assert_queue(main)` on macOS Sequoia and abort the process
// when called off the main thread. rdev cannot guarantee it is on it: the
// event-tap callback runs on whichever thread installed the tap, and
// `KeyboardState` is `Send`.
//
// Ceiling, paid only by `KeyboardState::add`: it has no real event, so it
// translates a synthetic one that was never posted, and that translation reads
// nothing the caller puts on the event. Measured on a US-layout macOS runner
// (subtrace-ai/subtrace run 30434587990): `CGEventSetFlags` is ignored, so
// shift and caps lock no longer reach the keycode translation the way
// `UCKeyTranslate`'s `modifier_state` did. Dead-key sequences ("´" + "e" → "é")
// are likewise not composed across events, since no `dead_state` survives a
// call. `add` therefore yields the layout's base character for a keycode. What
// the `HIDSystemState` source contributes on a machine physically holding a
// modifier is untested. The event-tap path is unaffected — it reads the real
// event, which the window server already translated.
use crate::macos::keycodes::code_from_key;
use crate::rdev::{EventType, Key, KeyboardState};
use core_foundation::string::UniChar;
use core_graphics::event::CGEvent;
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

pub struct Keyboard;

impl Keyboard {
    pub fn new() -> Option<Keyboard> {
        Some(Keyboard)
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

    /// Translates `code` through a synthetic event that is never posted. The
    /// translation reads only the keycode and the active layout — see the
    /// module comment's ceiling — so there is no modifier state to pass in.
    pub(crate) unsafe fn string_for_key(code: u32) -> Option<String> {
        let code: u16 = code.try_into().ok()?;
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
        let event = CGEvent::new_keyboard_event(source, code, true).ok()?;
        Self::string_from_event(&event)
    }
}

impl KeyboardState for Keyboard {
    fn add(&mut self, event_type: &EventType) -> Option<String> {
        match event_type {
            EventType::KeyPress(Key::ShiftLeft | Key::ShiftRight | Key::CapsLock) => None,
            EventType::KeyPress(key) => {
                let code = code_from_key(*key)?;
                unsafe { Self::string_for_key(code.into()) }
            }
            _ => None,
        }
    }

    fn reset(&mut self) {}
}
