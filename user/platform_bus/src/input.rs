//! Human Interface Devices (HIDs) are devices that users interact with to provide input to the
//! computer. They can be connected to the platform via a variety of buses, and so we model them
//! abstractly as standard Platform Bus devices.

use ptah::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum InputEvent {
    KeyPressed { key: Key, state: KeyState },
    KeyReleased { key: Key, state: KeyState },
    RelX(i32),
    RelY(i32),
    RelZ(i32),
    RelWheel(i32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Key {
    /*
     * Buttons
     */
    BtnLeft,
    BtnMiddle,
    BtnRight,
    BtnSide,
    BtnExtra,

    /*
     * Keys
     */
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    KeyReturn,
    KeyEscape,
    KeyDelete,
    KeyTab,
    KeySpace,
    KeyDash,
    KeyEquals,
    KeyLeftBracket,
    KeyRightBracket,
    KeyForwardSlash,
    KeyPound,
    KeySemicolon,
    KeyApostrophe,
    KeyGrave,
    KeyComma,
    KeyDot,
    KeyBackSlash,
    KeyCapslock,
    KeyF1,
    KeyF2,
    KeyF3,
    KeyF4,
    KeyF5,
    KeyF6,
    KeyF7,
    KeyF8,
    KeyF9,
    KeyF10,
    KeyF11,
    KeyF12,
    KeyPrintScreen,
    KeyScrolllock,
    KeyPause,
    KeyInsert,
    KeyHome,
    KeyPageUp,
    KeyDeleteForward,
    KeyEnd,
    KeyPageDown,
    KeyRightArrow,
    KeyLeftArrow,
    KeyDownArrow,
    KeyUpArrow,
    KeyNumlock,
    KeypadSlash,
    KeypadAsterix,
    KeypadDash,
    KeypadPlus,
    KeypadEnter,
    Keypad1,
    Keypad2,
    Keypad3,
    Keypad4,
    Keypad5,
    Keypad6,
    Keypad7,
    Keypad8,
    Keypad9,
    Keypad0,
    KeypadDot,
    KeypadNonUsBackSlash,
    KeyApplication,
    KeyPower,
    KeypadEquals,
    KeyF13,
    KeyF14,
    KeyF15,
    KeyF16,
    KeyF17,
    KeyF18,
    KeyF19,
    KeyF20,
    KeyF21,
    KeyF22,
    KeyF23,
    KeyF24,
    KeyExecute,
    KeyHelp,
    KeyMenu,
    KeySelect,
    KeyStop,
    KeyAgain,
    KeyUndo,
    KeyCut,
    KeyCopy,
    KeyPaste,
    KeyFind,
    KeyMute,
    KeyVolumeUp,
    KeyVolumeDown,
    KeyLockingCapslock,
    KeyLockingNumlock,
    KeyLockingScrolllock,
    KeypadComma,
    // TODO: a bunch missing here bc I got bored
    KeyLeftControl,
    KeyLeftShift,
    KeyLeftAlt,
    KeyLeftGui,
    KeyRightControl,
    KeyRightShift,
    KeyRightAlt,
    KeyRightGui,
}

/// Represents the state of the modifier keys when another key is pressed. We differentiate between
/// left and right modifier keys (see methods on `KeyState` to test if either of a modifier key is
/// active).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub struct KeyState {
    pub left_ctrl: bool,
    pub left_shift: bool,
    pub left_alt: bool,
    pub left_gui: bool,

    pub right_ctrl: bool,
    pub right_shift: bool,
    pub right_alt: bool,
    pub right_gui: bool,
}

impl KeyState {
    pub fn ctrl(&self) -> bool {
        self.left_ctrl || self.right_ctrl
    }

    pub fn shift(&self) -> bool {
        self.left_shift || self.right_shift
    }

    pub fn alt(&self) -> bool {
        self.left_alt || self.right_alt
    }

    pub fn gui(&self) -> bool {
        self.left_gui || self.right_gui
    }
}
