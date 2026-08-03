//! Shared physical-input queries and synthetic mouse output.

use std::fmt;
use std::mem::size_of;
use std::sync::RwLock;
use std::time::Duration;

use device_query::{DeviceQuery, DeviceState, Keycode, MouseState};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_MOVE_NOCOALESCE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, SendInput,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InputBackend {
    #[default]
    Raskal,
    Windows,
}

impl InputBackend {
    pub fn from_name(value: &str) -> Self {
        if value.eq_ignore_ascii_case("windows") {
            Self::Windows
        } else {
            Self::Raskal
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InputError(String);

impl InputError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InputError {}

type MoveCallback = dyn Fn(i32, i32) -> Result<(), String> + Send + Sync;
type ClickCallback = dyn Fn(MouseButton, Duration) -> Result<(), String> + Send + Sync;

struct RaskalBackend {
    move_relative: Box<MoveCallback>,
    click: Box<ClickCallback>,
}

/// One application-wide input handle shared by frame logic and worker threads.
pub struct InputDeviceState {
    query: DeviceState,
    raskal: RwLock<Option<RaskalBackend>>,
}

impl Default for InputDeviceState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputDeviceState {
    pub fn new() -> Self {
        Self {
            query: DeviceState::new(),
            raskal: RwLock::new(None),
        }
    }

    pub fn set_raskal_backend<M, C>(&self, move_relative: M, click: C)
    where
        M: Fn(i32, i32) -> Result<(), String> + Send + Sync + 'static,
        C: Fn(MouseButton, Duration) -> Result<(), String> + Send + Sync + 'static,
    {
        *self.raskal.write().unwrap_or_else(|error| error.into_inner()) = Some(RaskalBackend {
            move_relative: Box::new(move_relative),
            click: Box::new(click),
        });
    }

    pub fn keys(&self) -> Vec<Keycode> {
        self.query.get_keys()
    }

    pub fn mouse(&self) -> MouseState {
        self.query.get_mouse()
    }

    pub fn key_down(&self, key: Keycode) -> bool {
        self.keys().contains(&key)
    }

    pub fn mouse_button_down(&self, button: MouseButton) -> bool {
        let index = match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            MouseButton::Middle => 3,
            MouseButton::Button4 => 4,
            MouseButton::Button5 => 5,
        };
        self.mouse().button_pressed.get(index).copied().unwrap_or(false)
    }

    pub fn move_mouse(&self, backend: InputBackend, dx: i32, dy: i32) -> Result<(), InputError> {
        if dx == 0 && dy == 0 {
            return Ok(());
        }
        match backend {
            InputBackend::Raskal => {
                let raskal = self.raskal.read().unwrap_or_else(|error| error.into_inner());
                let raskal = raskal.as_ref().ok_or_else(|| InputError::new("Raskal backend is not registered"))?;
                (raskal.move_relative)(dx, dy).map_err(InputError::new)
            }
            InputBackend::Windows => windows_move(dx, dy),
        }
    }

    pub fn click(&self, backend: InputBackend, button: MouseButton, hold: Duration) -> Result<(), InputError> {
        match backend {
            InputBackend::Raskal => {
                let raskal = self.raskal.read().unwrap_or_else(|error| error.into_inner());
                let raskal = raskal.as_ref().ok_or_else(|| InputError::new("Raskal backend is not registered"))?;
                (raskal.click)(button, hold).map_err(InputError::new)
            }
            InputBackend::Windows => windows_click(button, hold),
        }
    }
}

fn windows_move(dx: i32, dy: i32) -> Result<(), InputError> {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_MOVE_NOCOALESCE,
                ..Default::default()
            },
        },
    };
    (unsafe { SendInput(&[input], size_of::<INPUT>() as i32) } == 1)
        .then_some(())
        .ok_or_else(|| InputError::new("Windows SendInput move failed"))
}

fn windows_click(button: MouseButton, hold: Duration) -> Result<(), InputError> {
    let (down, up, data) = match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0),
        MouseButton::Button4 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 1),
        MouseButton::Button5 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 2),
    };
    let send = |flags, mouse_data| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT { mouseData: mouse_data, dwFlags: flags, ..Default::default() },
        },
    };
    if unsafe { SendInput(&[send(down, data)], size_of::<INPUT>() as i32) } != 1 {
        return Err(InputError::new("Windows SendInput button-down failed"));
    }
    if !hold.is_zero() {
        std::thread::sleep(hold);
    }
    (unsafe { SendInput(&[send(up, data)], size_of::<INPUT>() as i32) } == 1)
        .then_some(())
        .ok_or_else(|| InputError::new("Windows SendInput button-up failed"))
}
