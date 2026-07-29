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
// The shifted case pins the fork's modifier ceiling rather than a fix. Apple
// does not document whether `CGEventKeyboardGetUnicodeString` honours flags on
// an unposted synthetic event; measurement says it does not (subtrace-ai/
// subtrace run 30434587990). `"A"` here would mean the ceiling has lifted and
// `KeyboardState` can carry modifier state again.

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
        Some("a"),
        "known ceiling: shift does not reach an unposted synthetic event's translation"
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("subtrace_offmain_layout probe is macOS-only");
}
