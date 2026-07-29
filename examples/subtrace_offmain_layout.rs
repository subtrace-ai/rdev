// SUBTRACE FORK regression probe for the macOS Sequoia keystroke crash.
//
// Before the common.rs patch, rdev derived a key's name inside its event-tap
// callback via the Text Input Source APIs, which abort with
// `dispatch_assert_queue(main)` when called off the main thread — and rdev's
// callback runs on a background thread in this app, so every keystroke crashed.
// The fix reads the Unicode string the window server already stored on the
// event, via `CGEventKeyboardGetUnicodeString`, which is safe off the main
// thread. This probe exercises exactly that call from a background thread.
//
// Run on macOS:  cargo run --example subtrace_offmain_layout
// Exit 0 = fixed. A crash / non-zero exit = regression.

#[cfg(target_os = "macos")]
fn main() {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use foreign_types::ForeignType;
    use std::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventKeyboardGetUnicodeString(
            event: *mut c_void,
            max_string_length: usize,
            actual_string_length: *mut usize,
            unicode_string: *mut u16,
        );
    }

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .expect("invariant: creating a private CGEventSource never needs permissions");
    let event = CGEvent::new_keyboard_event(source, 0, true)
        .expect("invariant: constructing a keyboard CGEvent succeeds");
    event.set_string("a");

    let event_ptr = event.as_ptr() as usize;
    let translated = std::thread::spawn(move || {
        let mut buff = [0_u16; 8];
        let mut length: usize = 0;
        // SAFETY: `event_ptr` points at the CGEvent owned by `main`, kept alive
        // until this thread is joined below; the buffer bounds match the call.
        unsafe {
            CGEventKeyboardGetUnicodeString(
                event_ptr as *mut c_void,
                buff.len(),
                &mut length as *mut usize,
                buff.as_mut_ptr(),
            );
            String::from_utf16(&buff[..length]).ok()
        }
    })
    .join()
    .expect("probe worker thread panicked");

    drop(event);

    assert_eq!(
        translated.as_deref(),
        Some("a"),
        "off-main-thread key string extraction must succeed without crashing"
    );
    println!("off-main-thread extraction OK: {translated:?}");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("subtrace_offmain_layout probe is macOS-only");
}
