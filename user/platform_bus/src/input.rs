//! Human Interface Devices (HIDs) are devices that users interact with to provide input to the
//! computer. They can be connected to the platform via a variety of buses, and so we model them
//! abstractly as standard Platform Bus devices.

use ptah::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum InputEvent {
    // TODO: enumerate keys instead of turning them into chars here - this is the PHYSICAL layer
    // and then should go through a keymap later
    KeyPressed { key: char, state: KeyState },
    KeyReleased { key: char, state: KeyState },
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
