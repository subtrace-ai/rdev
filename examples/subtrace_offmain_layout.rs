// SUBTRACE FORK regression probe for the macOS Sequoia keystroke crash.
//
// Before the patch, rdev derived a key's name via the Text Input Source APIs,
// which abort with `dispatch_assert_queue(main)` when called off the main
// thread — and both rdev's event-tap callback and any background
// `KeyboardState` user run there, so every keystroke crashed the process. The
// fork resolves names through `CGEventKeyboardGetUnicodeString`, which touches
// no input-source state.
//
// Run on macOS with a US layout:  cargo run --example subtrace_offmain_layout
// Exit 0 = fixed. A crash / non-zero exit = regression.
//
// The shifted case is the open question Apple's docs do not answer:
// `CGEventKeyboardGetUnicodeString` translates an unposted synthetic event from
// its virtual keycode, and whether it honours flags set via `CGEventSetFlags`
// is undocumented. This probe answers it on real hardware.

#[cfg(target_os = "macos")]
fn main() {
    use rdev::{EventType, Key, Keyboard, KeyboardState};

    let (plain, shifted) = std::thread::spawn(|| {
        let mut keyboard = Keyboard::new().expect("invariant: Keyboard::new never fails on macOS");
        let plain = keyboard.add(&EventType::KeyPress(Key::KeyA));
        keyboard.add(&EventType::KeyPress(Key::ShiftLeft));
        let shifted = keyboard.add(&EventType::KeyPress(Key::KeyA));
        (plain, shifted)
    })
    .join()
    .expect("probe worker thread panicked");

    println!("off-main key names: plain={plain:?} shifted={shifted:?}");
    assert_eq!(
        plain.as_deref(),
        Some("a"),
        "off-main key-name resolution must succeed without crashing"
    );
    assert_eq!(
        shifted.as_deref(),
        Some("A"),
        "shift flags must reach the synthetic event's translation"
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("subtrace_offmain_layout probe is macOS-only");
}
