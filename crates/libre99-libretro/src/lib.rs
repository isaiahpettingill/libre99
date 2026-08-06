// MIT License
//
// Copyright (c) 2026 Isaiah Pettingill
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//
// This file is the Isaiah Pettingill libretro adapter. It links to Libre99
// emulator and firmware components authored by Joel Odom; those components keep
// their original copyright and license, and no rights in them are granted by
// this adapter license. All rights in Joel Odom's portion remain reserved to
// Joel Odom.

//! Libre99's libretro adapter.
//!
//! The emulator core stays safe and dependency-free. This crate is the small
//! unsafe boundary that presents it as a libretro ABI 1 dynamic library. It
//! accepts no-content boot, `.ctg`/raw `.bin` cartridges, and `.dsk` disk images;
//! the clean-room firmware is built in, while files in the libretro system
//! directory can override each firmware component.

#![allow(clippy::missing_safety_doc)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::ptr;

use libre99_core::cartridge::Cartridge;
use libre99_core::keyboard::TiKey;
use libre99_core::machine::Machine;
use libre99_core::vdp::{HEIGHT, WIDTH};

const RETRO_API_VERSION: u32 = 1;
const RETRO_REGION_NTSC: u32 = 0;
const RETRO_PIXEL_FORMAT_XRGB8888: u32 = 1;
const RETRO_PIXEL_FORMAT_RGB565: u32 = 2;

const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: u32 = 9;
const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: u32 = 10;
const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: u32 = 11;
const RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK: u32 = 12;
const RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE: u32 = 13;
const RETRO_ENVIRONMENT_GET_VARIABLE: u32 = 15;
const RETRO_ENVIRONMENT_SET_VARIABLES: u32 = 16;
const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: u32 = 17;
const RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME: u32 = 18;

const RETRO_DEVICE_JOYPAD: u32 = 1;
const RETRO_DEVICE_MOUSE: u32 = 2;
const RETRO_DEVICE_KEYBOARD: u32 = 3;

const RETRO_DEVICE_ID_JOYPAD_B: u32 = 0;
const RETRO_DEVICE_ID_JOYPAD_Y: u32 = 1;
const RETRO_DEVICE_ID_JOYPAD_SELECT: u32 = 2;
const RETRO_DEVICE_ID_JOYPAD_START: u32 = 3;
const RETRO_DEVICE_ID_JOYPAD_UP: u32 = 4;
const RETRO_DEVICE_ID_JOYPAD_DOWN: u32 = 5;
const RETRO_DEVICE_ID_JOYPAD_LEFT: u32 = 6;
const RETRO_DEVICE_ID_JOYPAD_RIGHT: u32 = 7;
const RETRO_DEVICE_ID_JOYPAD_A: u32 = 8;
const RETRO_DEVICE_ID_JOYPAD_X: u32 = 9;
const RETRO_DEVICE_ID_JOYPAD_L: u32 = 10;
const RETRO_DEVICE_ID_JOYPAD_R: u32 = 11;
const RETRO_DEVICE_ID_JOYPAD_L2: u32 = 12;
const RETRO_DEVICE_ID_JOYPAD_R2: u32 = 13;
const RETRO_DEVICE_ID_JOYPAD_L3: u32 = 14;
const RETRO_DEVICE_ID_JOYPAD_R3: u32 = 15;

const RETRO_DEVICE_ID_MOUSE_X: u32 = 0;
const RETRO_DEVICE_ID_MOUSE_Y: u32 = 1;
const RETRO_DEVICE_ID_MOUSE_LEFT: u32 = 2;
const RETRO_DEVICE_ID_MOUSE_RIGHT: u32 = 3;
const RETRO_DEVICE_ID_MOUSE_WHEELUP: u32 = 4;
const RETRO_DEVICE_ID_MOUSE_WHEELDOWN: u32 = 5;
const RETRO_DEVICE_ID_MOUSE_MIDDLE: u32 = 6;

const RETROK_BACKSPACE: u32 = 8;
const RETROK_RETURN: u32 = 13;
const RETROK_ESCAPE: u32 = 27;
const RETROK_SPACE: u32 = 32;
const RETROK_DELETE: u32 = 127;
const RETROK_UP: u32 = 273;
const RETROK_DOWN: u32 = 274;
const RETROK_RIGHT: u32 = 275;
const RETROK_LEFT: u32 = 276;
const RETROK_INSERT: u32 = 277;
const RETROK_HOME: u32 = 278;
const RETROK_END: u32 = 279;
const RETROK_LALT: u32 = 308;
const RETROK_RALT: u32 = 307;
const RETROK_LCTRL: u32 = 306;
const RETROK_RCTRL: u32 = 305;
const RETROK_LSHIFT: u32 = 304;
const RETROK_RSHIFT: u32 = 303;
const RETROK_EQUALS: u32 = b'=' as u32;
const RETROK_PERIOD: u32 = b'.' as u32;
const RETROK_COMMA: u32 = b',' as u32;
const RETROK_SEMICOLON: u32 = b';' as u32;
const RETROK_SLASH: u32 = b'/' as u32;

const RETROKMOD_SHIFT: u16 = 0x01;
const RETROKMOD_CTRL: u16 = 0x02;
const RETROKMOD_ALT: u16 = 0x04;
const RETROKMOD_META: u16 = 0x08;

const MAX_KEYCODE: usize = 512;
const AUDIO_RATE: u32 = 44_100;
const AUDIO_FRAMES: usize = 735;
const MAX_MEDIA_BYTES: usize = 8 * 1024 * 1024;
const SERIALIZE_SIZE: usize = 16 * 1024 * 1024;
const STATE_MAGIC: [u8; 8] = *b"L99STATE";
const STATE_VERSION: u32 = 1;

static LIBRARY_NAME: &[u8] = b"Libre99\0";
static LIBRARY_VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
static VALID_EXTENSIONS: &[u8] = b"ctg|bin|dsk\0";

#[derive(Clone, Copy, PartialEq, Eq)]
enum PixelFormat {
    Xrgb8888,
    Rgb565,
}

#[repr(C)]
pub struct RetroSystemInfo {
    library_name: *const c_char,
    library_version: *const c_char,
    valid_extensions: *const c_char,
    need_fullpath: bool,
    block_extract: bool,
}

#[repr(C)]
pub struct RetroGameInfo {
    path: *const c_char,
    data: *const c_void,
    size: usize,
    meta: *const c_char,
}

#[repr(C)]
struct RetroGameGeometry {
    base_width: u32,
    base_height: u32,
    max_width: u32,
    max_height: u32,
    aspect_ratio: f32,
}

#[repr(C)]
struct RetroSystemTiming {
    fps: f64,
    sample_rate: f64,
}

#[repr(C)]
pub struct RetroSystemAvInfo {
    geometry: RetroGameGeometry,
    timing: RetroSystemTiming,
}

#[repr(C)]
struct RetroVariable {
    key: *const c_char,
    value: *const c_char,
}

#[repr(C)]
struct RetroInputDescriptor {
    port: u32,
    device: u32,
    index: u32,
    id: u32,
    description: *const c_char,
}

#[repr(C)]
struct RetroKeyboardCallback {
    callback: Option<RetroKeyboardEventFn>,
}

#[repr(C)]
struct RetroDiskControlCallback {
    set_eject_state: Option<RetroSetEjectStateFn>,
    get_eject_state: Option<RetroGetEjectStateFn>,
    get_image_index: Option<RetroGetImageIndexFn>,
    set_image_index: Option<RetroSetImageIndexFn>,
    get_num_images: Option<RetroGetNumImagesFn>,
    replace_image_index: Option<RetroReplaceImageIndexFn>,
    add_image_index: Option<RetroAddImageIndexFn>,
}

type RetroEnvironmentFn = unsafe extern "C" fn(u32, *mut c_void) -> bool;
type RetroVideoRefreshFn = unsafe extern "C" fn(*const c_void, u32, u32, usize);
type RetroAudioSampleFn = unsafe extern "C" fn(i16, i16);
type RetroAudioSampleBatchFn = unsafe extern "C" fn(*const i16, usize) -> usize;
type RetroInputPollFn = unsafe extern "C" fn();
type RetroInputStateFn = unsafe extern "C" fn(u32, u32, u32, u32) -> i16;
type RetroKeyboardEventFn = unsafe extern "C" fn(bool, u32, u32, u16);
type RetroSetEjectStateFn = unsafe extern "C" fn(bool) -> bool;
type RetroGetEjectStateFn = unsafe extern "C" fn() -> bool;
type RetroGetImageIndexFn = unsafe extern "C" fn() -> u32;
type RetroSetImageIndexFn = unsafe extern "C" fn(u32) -> bool;
type RetroGetNumImagesFn = unsafe extern "C" fn() -> u32;
type RetroReplaceImageIndexFn = unsafe extern "C" fn(u32, *const RetroGameInfo) -> bool;
type RetroAddImageIndexFn = unsafe extern "C" fn() -> bool;

static DISK_CONTROL: RetroDiskControlCallback = RetroDiskControlCallback {
    set_eject_state: Some(disk_set_eject_state),
    get_eject_state: Some(disk_get_eject_state),
    get_image_index: Some(disk_get_image_index),
    set_image_index: Some(disk_set_image_index),
    get_num_images: Some(disk_get_num_images),
    replace_image_index: Some(disk_replace_image_index),
    add_image_index: Some(disk_add_image_index),
};

static KEYBOARD_CALLBACK: RetroKeyboardCallback = RetroKeyboardCallback {
    callback: Some(retro_keyboard_event),
};

static mut ENVIRONMENT: Option<RetroEnvironmentFn> = None;
static mut VIDEO_REFRESH: Option<RetroVideoRefreshFn> = None;
static mut AUDIO_SAMPLE: Option<RetroAudioSampleFn> = None;
static mut AUDIO_SAMPLE_BATCH: Option<RetroAudioSampleBatchFn> = None;
static mut INPUT_POLL: Option<RetroInputPollFn> = None;
static mut INPUT_STATE: Option<RetroInputStateFn> = None;
static mut PIXEL_FORMAT: PixelFormat = PixelFormat::Xrgb8888;
static mut KEYBOARD_CALLBACK_INSTALLED: bool = false;
static mut CORE: Option<Core> = None;

#[derive(Clone, Copy)]
struct KeyboardEventState {
    character: u32,
    modifiers: u16,
}

#[derive(Clone)]
struct DiskImage {
    key: Option<String>,
    label: String,
    bytes: Vec<u8>,
}

struct Firmware {
    rom: Vec<u8>,
    grom: Vec<u8>,
    dsr: Vec<u8>,
}

#[derive(Clone, Copy)]
struct InputConfig {
    p1: [Option<TiKey>; 16],
    p2: [Option<TiKey>; 16],
}

impl Default for InputConfig {
    fn default() -> Self {
        let mut p1 = [None; 16];
        let mut p2 = [None; 16];
        p1[RETRO_DEVICE_ID_JOYPAD_B as usize] = Some(TiKey::Space);
        p1[RETRO_DEVICE_ID_JOYPAD_Y as usize] = Some(TiKey::Enter);
        p1[RETRO_DEVICE_ID_JOYPAD_SELECT as usize] = Some(TiKey::Fctn);
        p1[RETRO_DEVICE_ID_JOYPAD_START as usize] = Some(TiKey::Ctrl);
        p1[RETRO_DEVICE_ID_JOYPAD_UP as usize] = Some(TiKey::Joy1Up);
        p1[RETRO_DEVICE_ID_JOYPAD_DOWN as usize] = Some(TiKey::Joy1Down);
        p1[RETRO_DEVICE_ID_JOYPAD_LEFT as usize] = Some(TiKey::Joy1Left);
        p1[RETRO_DEVICE_ID_JOYPAD_RIGHT as usize] = Some(TiKey::Joy1Right);
        p1[RETRO_DEVICE_ID_JOYPAD_A as usize] = Some(TiKey::Joy1Fire);
        p1[RETRO_DEVICE_ID_JOYPAD_X as usize] = Some(TiKey::Shift);
        p1[RETRO_DEVICE_ID_JOYPAD_L as usize] = Some(TiKey::Fctn);
        p1[RETRO_DEVICE_ID_JOYPAD_R as usize] = Some(TiKey::Ctrl);
        p1[RETRO_DEVICE_ID_JOYPAD_L2 as usize] = Some(TiKey::Joy1Fire);
        p1[RETRO_DEVICE_ID_JOYPAD_R2 as usize] = Some(TiKey::Joy1Fire);
        p1[RETRO_DEVICE_ID_JOYPAD_L3 as usize] = Some(TiKey::Joy1Fire);
        p1[RETRO_DEVICE_ID_JOYPAD_R3 as usize] = Some(TiKey::Joy1Fire);

        p2[RETRO_DEVICE_ID_JOYPAD_B as usize] = Some(TiKey::Space);
        p2[RETRO_DEVICE_ID_JOYPAD_Y as usize] = Some(TiKey::Enter);
        p2[RETRO_DEVICE_ID_JOYPAD_SELECT as usize] = Some(TiKey::Fctn);
        p2[RETRO_DEVICE_ID_JOYPAD_START as usize] = Some(TiKey::Ctrl);
        p2[RETRO_DEVICE_ID_JOYPAD_UP as usize] = Some(TiKey::Joy2Up);
        p2[RETRO_DEVICE_ID_JOYPAD_DOWN as usize] = Some(TiKey::Joy2Down);
        p2[RETRO_DEVICE_ID_JOYPAD_LEFT as usize] = Some(TiKey::Joy2Left);
        p2[RETRO_DEVICE_ID_JOYPAD_RIGHT as usize] = Some(TiKey::Joy2Right);
        p2[RETRO_DEVICE_ID_JOYPAD_A as usize] = Some(TiKey::Joy2Fire);
        p2[RETRO_DEVICE_ID_JOYPAD_X as usize] = Some(TiKey::Shift);
        p2[RETRO_DEVICE_ID_JOYPAD_L as usize] = Some(TiKey::Fctn);
        p2[RETRO_DEVICE_ID_JOYPAD_R as usize] = Some(TiKey::Ctrl);
        p2[RETRO_DEVICE_ID_JOYPAD_L2 as usize] = Some(TiKey::Joy2Fire);
        p2[RETRO_DEVICE_ID_JOYPAD_R2 as usize] = Some(TiKey::Joy2Fire);
        p2[RETRO_DEVICE_ID_JOYPAD_L3 as usize] = Some(TiKey::Joy2Fire);
        p2[RETRO_DEVICE_ID_JOYPAD_R3 as usize] = Some(TiKey::Joy2Fire);
        InputConfig { p1, p2 }
    }
}

struct Core {
    machine: Machine,
    framebuffer: Vec<u32>,
    framebuffer_565: Vec<u16>,
    audio_mono: Vec<f32>,
    audio_stereo: Vec<i16>,
    pixel_format: PixelFormat,
    keyboard_events: [Option<KeyboardEventState>; MAX_KEYCODE],
    keyboard_callback: bool,
    options: InputConfig,
    disk_images: Vec<DiskImage>,
    disk_index: usize,
    disk_ejected: bool,
}

impl Core {
    fn new(firmware: Firmware, pixel_format: PixelFormat, keyboard_callback: bool) -> Self {
        let mut machine = Machine::new(&firmware.rom, &firmware.grom);
        machine.load_disk_controller(&firmware.dsr);
        machine.set_audio_sample_rate(AUDIO_RATE);
        Core {
            machine,
            framebuffer: vec![0; WIDTH * HEIGHT],
            framebuffer_565: vec![0; WIDTH * HEIGHT],
            audio_mono: vec![0.0; AUDIO_FRAMES],
            audio_stereo: vec![0; AUDIO_FRAMES * 2],
            pixel_format,
            keyboard_events: [None; MAX_KEYCODE],
            keyboard_callback,
            options: InputConfig::default(),
            disk_images: Vec::new(),
            disk_index: 0,
            disk_ejected: false,
        }
    }

    fn refresh_options(&mut self, force: bool) {
        if !force && !variable_update() {
            return;
        }
        let defaults = InputConfig::default();
        let mut p1 = [None; 16];
        let mut p2 = [None; 16];
        for port_number in 1..=2 {
            let (port, default_port) = if port_number == 1 {
                (&mut p1, &defaults.p1)
            } else {
                (&mut p2, &defaults.p2)
            };
            for (id, default_key) in default_port.iter().enumerate() {
                let key = format!("libre99_p{port_number}_{}", joypad_name(id as u32));
                let value = option_value(&key);
                port[id] = match value.as_deref().map(str::trim) {
                    Some(value) if value.eq_ignore_ascii_case("none") => None,
                    Some(value) => parse_ti_key(value).or(*default_key),
                    None => *default_key,
                };
            }
        }
        self.options = InputConfig { p1, p2 };
    }

    fn apply_input(&mut self) {
        self.machine.bus_mut().keyboard.release_all();
        if self.keyboard_callback {
            for (keycode, event) in self
                .keyboard_events
                .iter()
                .enumerate()
                .filter_map(|(keycode, event)| event.map(|event| (keycode, event)))
            {
                for key in resolve_keyboard(keycode as u32, event.character, event.modifiers)
                    .into_iter()
                    .flatten()
                {
                    self.machine.set_key(key, true);
                }
            }
        } else {
            self.apply_polled_keyboard();
        }

        let Some(input) = input_state_callback() else {
            return;
        };
        for (port, mapping) in [(0, self.options.p1), (1, self.options.p2)] {
            for (id, key) in mapping.iter().enumerate() {
                if joypad_pressed(input, port, id as u32) {
                    if let Some(key) = key {
                        self.machine.set_key(*key, true);
                    }
                }
            }
        }

        let mouse_x = input_value(input, 0, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_X);
        let mouse_y = input_value(input, 0, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_Y);
        let p1 = self.options.p1;
        if mouse_x < 0 {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_LEFT as usize]);
        } else if mouse_x > 0 {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_RIGHT as usize]);
        }
        if mouse_y < 0 {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_UP as usize]);
        } else if mouse_y > 0 {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_DOWN as usize]);
        }
        if input_value(input, 0, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_LEFT) != 0 {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_A as usize]);
        }
        if input_value(input, 0, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_RIGHT) != 0 {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_B as usize]);
        }
        if input_value(input, 0, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_MIDDLE) != 0 {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_X as usize]);
        }
        if input_value(input, 0, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_WHEELUP) != 0 {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_Y as usize]);
        }
        if input_value(
            input,
            0,
            RETRO_DEVICE_MOUSE,
            RETRO_DEVICE_ID_MOUSE_WHEELDOWN,
        ) != 0
        {
            set_mapped(&mut self.machine, p1[RETRO_DEVICE_ID_JOYPAD_B as usize]);
        }
    }

    fn apply_polled_keyboard(&mut self) {
        let Some(input) = input_state_callback() else {
            return;
        };
        for b in b'a'..=b'z' {
            if keyboard_pressed(input, b as u32) {
                set_mapped(&mut self.machine, ascii_key(b as char));
            }
        }
        for b in b'0'..=b'9' {
            if keyboard_pressed(input, b as u32) {
                set_mapped(&mut self.machine, ascii_key(b as char));
            }
        }
        for (code, key) in [
            (b'=' as u32, TiKey::Equals),
            (b'.' as u32, TiKey::Period),
            (b',' as u32, TiKey::Comma),
            (b';' as u32, TiKey::Semicolon),
            (b'/' as u32, TiKey::Slash),
            (RETROK_SPACE, TiKey::Space),
            (RETROK_RETURN, TiKey::Enter),
            (RETROK_LSHIFT, TiKey::Shift),
            (RETROK_RSHIFT, TiKey::Shift),
            (RETROK_LCTRL, TiKey::Ctrl),
            (RETROK_RCTRL, TiKey::Ctrl),
            (RETROK_LALT, TiKey::Fctn),
            (RETROK_RALT, TiKey::Joy1Fire),
            (RETROK_UP, TiKey::Joy1Up),
            (RETROK_DOWN, TiKey::Joy1Down),
            (RETROK_LEFT, TiKey::Joy1Left),
            (RETROK_RIGHT, TiKey::Joy1Right),
            (RETROK_ESCAPE, TiKey::Fctn),
        ] {
            if keyboard_pressed(input, code) {
                self.machine.set_key(key, true);
            }
        }
        if keyboard_pressed(input, RETROK_BACKSPACE) {
            self.machine.set_key(TiKey::Fctn, true);
            self.machine.set_key(TiKey::S, true);
        }
        if keyboard_pressed(input, RETROK_DELETE) {
            self.machine.set_key(TiKey::Fctn, true);
            self.machine.set_key(TiKey::Num1, true);
        }
        if keyboard_pressed(input, RETROK_INSERT) {
            self.machine.set_key(TiKey::Fctn, true);
            self.machine.set_key(TiKey::Num2, true);
        }
        if keyboard_pressed(input, RETROK_HOME) {
            self.machine.set_key(TiKey::Fctn, true);
            self.machine.set_key(TiKey::Num5, true);
        }
        if keyboard_pressed(input, RETROK_END) {
            self.machine.set_key(TiKey::Fctn, true);
            self.machine.set_key(TiKey::Num6, true);
        }
    }

    fn mount_disk(&mut self, image: DiskImage) {
        self.sync_current_disk();
        self.disk_images = vec![image];
        self.disk_index = 0;
        self.disk_ejected = false;
        self.mount_selected_disk();
    }

    fn mount_content_disk(&mut self, image: DiskImage) {
        self.mount_disk(image);
        // A standalone disk is core content, so let the console see DSK1 during
        // its initial reset. Disk Control inserts remain live and do not reset.
        self.machine.reset();
    }

    fn mount_selected_disk(&mut self) {
        if self.disk_ejected {
            return;
        }
        let Some(image) = self.disk_images.get(self.disk_index).cloned() else {
            self.machine.eject_disk(0);
            return;
        };
        if let Some(key) = image.key.as_deref() {
            self.machine.mount_disk_keyed(0, key, image.bytes);
        } else {
            self.machine.mount_disk(0, image.bytes);
        }
    }

    fn sync_current_disk(&mut self) {
        let Some(image) = self.disk_images.get_mut(self.disk_index) else {
            return;
        };
        if !self.disk_ejected {
            if let Some(bytes) = self.machine.bus().disk.drive_image(0) {
                image.bytes = bytes.to_vec();
            }
        } else if let Some(key) = image.key.as_deref() {
            if let Some(bytes) = self.machine.bus().disk.image_for_key(key) {
                image.bytes = bytes.to_vec();
            }
        }
    }

    fn set_eject_state(&mut self, ejected: bool) -> bool {
        if ejected == self.disk_ejected {
            return true;
        }
        if ejected {
            self.sync_current_disk();
            self.machine.eject_disk(0);
            self.disk_ejected = true;
        } else {
            self.disk_ejected = false;
            self.mount_selected_disk();
        }
        true
    }

    fn replace_disk(&mut self, index: usize, image: Option<DiskImage>) -> bool {
        if let Some(image) = image {
            if image.bytes.is_empty() || image.bytes.len() > MAX_MEDIA_BYTES {
                return false;
            }
            if index >= self.disk_images.len() {
                return false;
            }
            if index == self.disk_index && !self.disk_ejected {
                self.sync_current_disk();
                let old_key = self.disk_images[index].key.clone();
                self.machine.eject_disk(0);
                if let Some(old_key) = old_key {
                    self.machine.bus_mut().disk.forget(&old_key);
                }
            }
            self.disk_images[index] = image;
            if index == self.disk_index && !self.disk_ejected {
                self.mount_selected_disk();
            }
            true
        } else {
            if index >= self.disk_images.len() {
                return false;
            }
            if index == self.disk_index && !self.disk_ejected {
                self.sync_current_disk();
                self.machine.eject_disk(0);
            }
            self.disk_images.remove(index);
            if self.disk_images.is_empty() {
                self.disk_index = 0;
                self.disk_ejected = true;
            } else {
                self.disk_index = self.disk_index.min(self.disk_images.len() - 1);
                if !self.disk_ejected {
                    self.mount_selected_disk();
                }
            }
            true
        }
    }

    fn add_disk(&mut self) -> bool {
        self.disk_images.push(DiskImage {
            key: None,
            label: format!("disk {}", self.disk_images.len() + 1),
            bytes: Vec::new(),
        });
        true
    }

    fn serialize_into(&mut self, dst: &mut [u8]) -> bool {
        self.sync_current_disk();
        let metadata = self.serialize_metadata();
        let machine = self.machine.save_state();
        let total = 20usize
            .checked_add(metadata.len())
            .and_then(|n| n.checked_add(machine.len()));
        let Some(total) = total else {
            return false;
        };
        if total > dst.len() || total > SERIALIZE_SIZE {
            return false;
        }
        dst.fill(0);
        dst[..8].copy_from_slice(&STATE_MAGIC);
        dst[8..12].copy_from_slice(&STATE_VERSION.to_le_bytes());
        dst[12..16].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
        dst[16..20].copy_from_slice(&(machine.len() as u32).to_le_bytes());
        dst[20..20 + metadata.len()].copy_from_slice(&metadata);
        dst[20 + metadata.len()..total].copy_from_slice(&machine);
        true
    }

    fn serialize_metadata(&self) -> Vec<u8> {
        let mut w = MetadataWriter::default();
        w.u32(self.disk_index as u32);
        w.u8(self.disk_ejected as u8);
        w.u32(self.disk_images.len() as u32);
        for image in &self.disk_images {
            w.optional_string(image.key.as_deref());
            w.string(&image.label);
            w.blob(&image.bytes);
        }
        w.bytes
    }

    fn unserialize(&mut self, data: &[u8]) -> bool {
        if data.len() < 20 || data.len() > SERIALIZE_SIZE || data[..8] != STATE_MAGIC {
            return false;
        }
        let version = u32::from_le_bytes(data[8..12].try_into().unwrap());
        if version != STATE_VERSION {
            return false;
        }
        let metadata_len = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let machine_len = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
        let Some(metadata_end) = 20usize.checked_add(metadata_len) else {
            return false;
        };
        let Some(end) = metadata_end.checked_add(machine_len) else {
            return false;
        };
        if end > data.len() {
            return false;
        }
        let Some((disk_index, disk_ejected, disk_images)) =
            MetadataReader::new(&data[20..metadata_end]).read_all()
        else {
            return false;
        };
        if !disk_images.is_empty() && disk_index >= disk_images.len() {
            return false;
        }
        if self.machine.load_state(&data[metadata_end..end]).is_err() {
            return false;
        }
        self.disk_index = disk_index;
        self.disk_ejected = disk_ejected;
        self.disk_images = disk_images;
        true
    }
}

#[derive(Default)]
struct MetadataWriter {
    bytes: Vec<u8>,
}

impl MetadataWriter {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }
    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn blob(&mut self, value: &[u8]) {
        self.u32(value.len() as u32);
        self.bytes.extend_from_slice(value);
    }
    fn string(&mut self, value: &str) {
        self.blob(value.as_bytes());
    }
    fn optional_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.string(value);
            }
            None => self.u8(0),
        }
    }
}

struct MetadataReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> MetadataReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        MetadataReader { data, pos: 0 }
    }
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(count)?;
        let bytes = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(bytes)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|bytes| bytes[0])
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn blob(&mut self) -> Option<Vec<u8>> {
        let len = self.u32()? as usize;
        if len > MAX_MEDIA_BYTES {
            return None;
        }
        Some(self.take(len)?.to_vec())
    }
    fn string(&mut self) -> Option<String> {
        Some(String::from_utf8_lossy(&self.blob()?).into_owned())
    }
    fn optional_string(&mut self) -> Option<Option<String>> {
        if self.u8()? == 0 {
            Some(None)
        } else {
            Some(Some(self.string()?))
        }
    }
    fn read_all(mut self) -> Option<(usize, bool, Vec<DiskImage>)> {
        let disk_index = self.u32()? as usize;
        let disk_ejected = self.u8()? != 0;
        let count = self.u32()? as usize;
        if count > 64 {
            return None;
        }
        let mut images = Vec::with_capacity(count);
        for _ in 0..count {
            let key = self.optional_string()?;
            let label = self.string()?;
            let bytes = self.blob()?;
            images.push(DiskImage { key, label, bytes });
        }
        if self.pos != self.data.len() {
            return None;
        }
        Some((disk_index, disk_ejected, images))
    }
}

fn set_mapped(machine: &mut Machine, key: Option<TiKey>) {
    if let Some(key) = key {
        machine.set_key(key, true);
    }
}

fn input_state_callback() -> Option<RetroInputStateFn> {
    unsafe { INPUT_STATE }
}

fn joypad_pressed(input: RetroInputStateFn, port: u32, id: u32) -> bool {
    input_value(input, port, RETRO_DEVICE_JOYPAD, id) != 0
}

fn keyboard_pressed(input: RetroInputStateFn, id: u32) -> bool {
    input_value(input, 0, RETRO_DEVICE_KEYBOARD, id) != 0
}

fn input_value(input: RetroInputStateFn, port: u32, device: u32, id: u32) -> i16 {
    unsafe { input(port, device, 0, id) }
}

fn ascii_key(c: char) -> Option<TiKey> {
    Some(match c.to_ascii_uppercase() {
        'A' => TiKey::A,
        'B' => TiKey::B,
        'C' => TiKey::C,
        'D' => TiKey::D,
        'E' => TiKey::E,
        'F' => TiKey::F,
        'G' => TiKey::G,
        'H' => TiKey::H,
        'I' => TiKey::I,
        'J' => TiKey::J,
        'K' => TiKey::K,
        'L' => TiKey::L,
        'M' => TiKey::M,
        'N' => TiKey::N,
        'O' => TiKey::O,
        'P' => TiKey::P,
        'Q' => TiKey::Q,
        'R' => TiKey::R,
        'S' => TiKey::S,
        'T' => TiKey::T,
        'U' => TiKey::U,
        'V' => TiKey::V,
        'W' => TiKey::W,
        'X' => TiKey::X,
        'Y' => TiKey::Y,
        'Z' => TiKey::Z,
        '0' => TiKey::Num0,
        '1' => TiKey::Num1,
        '2' => TiKey::Num2,
        '3' => TiKey::Num3,
        '4' => TiKey::Num4,
        '5' => TiKey::Num5,
        '6' => TiKey::Num6,
        '7' => TiKey::Num7,
        '8' => TiKey::Num8,
        '9' => TiKey::Num9,
        '=' => TiKey::Equals,
        '.' => TiKey::Period,
        ',' => TiKey::Comma,
        ';' => TiKey::Semicolon,
        '/' => TiKey::Slash,
        ' ' => TiKey::Space,
        _ => return None,
    })
}

fn char_to_ti_press(c: char) -> [Option<TiKey>; 2] {
    let base = |key| [None, Some(key)];
    let shift = |key| [Some(TiKey::Shift), Some(key)];
    let fctn = |key| [Some(TiKey::Fctn), Some(key)];
    if c.is_ascii_alphabetic() {
        return [c.is_ascii_uppercase().then_some(TiKey::Shift), ascii_key(c)];
    }
    match c {
        '0' => base(TiKey::Num0),
        '1' => base(TiKey::Num1),
        '2' => base(TiKey::Num2),
        '3' => base(TiKey::Num3),
        '4' => base(TiKey::Num4),
        '5' => base(TiKey::Num5),
        '6' => base(TiKey::Num6),
        '7' => base(TiKey::Num7),
        '8' => base(TiKey::Num8),
        '9' => base(TiKey::Num9),
        '=' => base(TiKey::Equals),
        '.' => base(TiKey::Period),
        ',' => base(TiKey::Comma),
        ';' => base(TiKey::Semicolon),
        '/' => base(TiKey::Slash),
        ' ' => base(TiKey::Space),
        '!' => shift(TiKey::Num1),
        '@' => shift(TiKey::Num2),
        '#' => shift(TiKey::Num3),
        '$' => shift(TiKey::Num4),
        '%' => shift(TiKey::Num5),
        '^' => shift(TiKey::Num6),
        '&' => shift(TiKey::Num7),
        '*' => shift(TiKey::Num8),
        '(' => shift(TiKey::Num9),
        ')' => shift(TiKey::Num0),
        '+' => shift(TiKey::Equals),
        ':' => shift(TiKey::Semicolon),
        '<' => shift(TiKey::Comma),
        '>' => shift(TiKey::Period),
        '-' => shift(TiKey::Slash),
        '?' => fctn(TiKey::I),
        '_' => fctn(TiKey::U),
        '\'' => fctn(TiKey::O),
        '"' => fctn(TiKey::P),
        '~' => fctn(TiKey::W),
        '[' => fctn(TiKey::R),
        ']' => fctn(TiKey::T),
        '{' => fctn(TiKey::F),
        '}' => fctn(TiKey::G),
        '\\' => fctn(TiKey::Z),
        '|' => fctn(TiKey::A),
        '`' => fctn(TiKey::C),
        _ => [None; 2],
    }
}

fn resolve_keyboard(keycode: u32, character: u32, modifiers: u16) -> [Option<TiKey>; 2] {
    let physical = match keycode {
        RETROK_LSHIFT | RETROK_RSHIFT => [None, Some(TiKey::Shift)],
        RETROK_LCTRL | RETROK_RCTRL => [None, Some(TiKey::Ctrl)],
        RETROK_LALT => [None, Some(TiKey::Fctn)],
        RETROK_RALT => [None, Some(TiKey::Joy1Fire)],
        RETROK_UP => [None, Some(TiKey::Joy1Up)],
        RETROK_DOWN => [None, Some(TiKey::Joy1Down)],
        RETROK_LEFT => [None, Some(TiKey::Joy1Left)],
        RETROK_RIGHT => [None, Some(TiKey::Joy1Right)],
        RETROK_RETURN => [None, Some(TiKey::Enter)],
        RETROK_SPACE => [None, Some(TiKey::Space)],
        RETROK_BACKSPACE => [Some(TiKey::Fctn), Some(TiKey::S)],
        RETROK_DELETE => [Some(TiKey::Fctn), Some(TiKey::Num1)],
        RETROK_INSERT => [Some(TiKey::Fctn), Some(TiKey::Num2)],
        RETROK_HOME => [Some(TiKey::Fctn), Some(TiKey::Num5)],
        RETROK_END => [Some(TiKey::Fctn), Some(TiKey::Num6)],
        RETROK_ESCAPE => [Some(TiKey::Fctn), Some(TiKey::Equals)],
        _ => [None, None],
    };
    if physical != [None, None] {
        return physical;
    }
    if modifiers & (RETROKMOD_CTRL | RETROKMOD_ALT | RETROKMOD_META) != 0 {
        return [None, keycode_to_ti(keycode)];
    }
    if modifiers & RETROKMOD_SHIFT != 0 {
        return [Some(TiKey::Shift), keycode_to_ti(keycode)];
    }
    if let Some(character) = char::from_u32(character) {
        let press = char_to_ti_press(character);
        if press != [None; 2] {
            return press;
        }
    }
    [None, keycode_to_ti(keycode)]
}

fn keycode_to_ti(keycode: u32) -> Option<TiKey> {
    if (b'a' as u32..=b'z' as u32).contains(&keycode) {
        return ascii_key(char::from_u32(keycode).unwrap());
    }
    if (b'0' as u32..=b'9' as u32).contains(&keycode) {
        return ascii_key(char::from_u32(keycode).unwrap());
    }
    match keycode {
        RETROK_EQUALS => Some(TiKey::Equals),
        RETROK_PERIOD => Some(TiKey::Period),
        RETROK_COMMA => Some(TiKey::Comma),
        RETROK_SEMICOLON => Some(TiKey::Semicolon),
        RETROK_SLASH => Some(TiKey::Slash),
        _ => None,
    }
}

fn all_key_names() -> Vec<&'static str> {
    vec![
        "none",
        "a",
        "b",
        "c",
        "d",
        "e",
        "f",
        "g",
        "h",
        "i",
        "j",
        "k",
        "l",
        "m",
        "n",
        "o",
        "p",
        "q",
        "r",
        "s",
        "t",
        "u",
        "v",
        "w",
        "x",
        "y",
        "z",
        "num0",
        "num1",
        "num2",
        "num3",
        "num4",
        "num5",
        "num6",
        "num7",
        "num8",
        "num9",
        "equals",
        "period",
        "comma",
        "semicolon",
        "slash",
        "space",
        "enter",
        "fctn",
        "shift",
        "ctrl",
        "joy1_fire",
        "joy1_left",
        "joy1_right",
        "joy1_down",
        "joy1_up",
        "joy2_fire",
        "joy2_left",
        "joy2_right",
        "joy2_down",
        "joy2_up",
    ]
}

fn parse_ti_key(value: &str) -> Option<TiKey> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => None,
        "a" => Some(TiKey::A),
        "b" => Some(TiKey::B),
        "c" => Some(TiKey::C),
        "d" => Some(TiKey::D),
        "e" => Some(TiKey::E),
        "f" => Some(TiKey::F),
        "g" => Some(TiKey::G),
        "h" => Some(TiKey::H),
        "i" => Some(TiKey::I),
        "j" => Some(TiKey::J),
        "k" => Some(TiKey::K),
        "l" => Some(TiKey::L),
        "m" => Some(TiKey::M),
        "n" => Some(TiKey::N),
        "o" => Some(TiKey::O),
        "p" => Some(TiKey::P),
        "q" => Some(TiKey::Q),
        "r" => Some(TiKey::R),
        "s" => Some(TiKey::S),
        "t" => Some(TiKey::T),
        "u" => Some(TiKey::U),
        "v" => Some(TiKey::V),
        "w" => Some(TiKey::W),
        "x" => Some(TiKey::X),
        "y" => Some(TiKey::Y),
        "z" => Some(TiKey::Z),
        "num0" => Some(TiKey::Num0),
        "num1" => Some(TiKey::Num1),
        "num2" => Some(TiKey::Num2),
        "num3" => Some(TiKey::Num3),
        "num4" => Some(TiKey::Num4),
        "num5" => Some(TiKey::Num5),
        "num6" => Some(TiKey::Num6),
        "num7" => Some(TiKey::Num7),
        "num8" => Some(TiKey::Num8),
        "num9" => Some(TiKey::Num9),
        "equals" => Some(TiKey::Equals),
        "period" => Some(TiKey::Period),
        "comma" => Some(TiKey::Comma),
        "semicolon" => Some(TiKey::Semicolon),
        "slash" => Some(TiKey::Slash),
        "space" => Some(TiKey::Space),
        "enter" => Some(TiKey::Enter),
        "fctn" => Some(TiKey::Fctn),
        "shift" => Some(TiKey::Shift),
        "ctrl" => Some(TiKey::Ctrl),
        "joy1_fire" => Some(TiKey::Joy1Fire),
        "joy1_left" => Some(TiKey::Joy1Left),
        "joy1_right" => Some(TiKey::Joy1Right),
        "joy1_down" => Some(TiKey::Joy1Down),
        "joy1_up" => Some(TiKey::Joy1Up),
        "joy2_fire" => Some(TiKey::Joy2Fire),
        "joy2_left" => Some(TiKey::Joy2Left),
        "joy2_right" => Some(TiKey::Joy2Right),
        "joy2_down" => Some(TiKey::Joy2Down),
        "joy2_up" => Some(TiKey::Joy2Up),
        _ => None,
    }
}

const JOYPAD_NAMES: [(u32, &str); 16] = [
    (RETRO_DEVICE_ID_JOYPAD_B, "b"),
    (RETRO_DEVICE_ID_JOYPAD_Y, "y"),
    (RETRO_DEVICE_ID_JOYPAD_SELECT, "select"),
    (RETRO_DEVICE_ID_JOYPAD_START, "start"),
    (RETRO_DEVICE_ID_JOYPAD_UP, "up"),
    (RETRO_DEVICE_ID_JOYPAD_DOWN, "down"),
    (RETRO_DEVICE_ID_JOYPAD_LEFT, "left"),
    (RETRO_DEVICE_ID_JOYPAD_RIGHT, "right"),
    (RETRO_DEVICE_ID_JOYPAD_A, "a"),
    (RETRO_DEVICE_ID_JOYPAD_X, "x"),
    (RETRO_DEVICE_ID_JOYPAD_L, "l"),
    (RETRO_DEVICE_ID_JOYPAD_R, "r"),
    (RETRO_DEVICE_ID_JOYPAD_L2, "l2"),
    (RETRO_DEVICE_ID_JOYPAD_R2, "r2"),
    (RETRO_DEVICE_ID_JOYPAD_L3, "l3"),
    (RETRO_DEVICE_ID_JOYPAD_R3, "r3"),
];

fn joypad_name(id: u32) -> &'static str {
    JOYPAD_NAMES
        .iter()
        .find(|(known, _)| *known == id)
        .map_or("unknown", |(_, name)| name)
}

fn default_mapping_name(port: usize, id: usize) -> &'static str {
    match (port, id) {
        (1, 0) => "space",
        (1, 1) => "enter",
        (1, 2) => "fctn",
        (1, 3) => "ctrl",
        (1, 4) => "joy1_up",
        (1, 5) => "joy1_down",
        (1, 6) => "joy1_left",
        (1, 7) => "joy1_right",
        (1, 8) => "joy1_fire",
        (1, 9) => "shift",
        (1, 10) => "fctn",
        (1, 11) => "ctrl",
        (2, 0) => "space",
        (2, 1) => "enter",
        (2, 2) => "fctn",
        (2, 3) => "ctrl",
        (2, 4) => "joy2_up",
        (2, 5) => "joy2_down",
        (2, 6) => "joy2_left",
        (2, 7) => "joy2_right",
        (2, 8) => "joy2_fire",
        (2, 9) => "shift",
        (2, 10) => "fctn",
        (2, 11) => "ctrl",
        _ => {
            if port == 1 {
                "joy1_fire"
            } else {
                "joy2_fire"
            }
        }
    }
}

fn leak_c_string(value: String) -> *const c_char {
    Box::leak(
        CString::new(value)
            .expect("libretro metadata contains no NUL")
            .into_boxed_c_str(),
    )
    .as_ptr()
}

unsafe fn register_core_options(env: RetroEnvironmentFn) {
    let names = all_key_names();
    let mut variables = Vec::with_capacity(33);
    for port in 1..=2 {
        for (id, name) in JOYPAD_NAMES {
            let key = format!("libre99_p{port}_{name}");
            let default = default_mapping_name(port, id as usize);
            let rest = names
                .iter()
                .copied()
                .filter(|value| *value != default)
                .collect::<Vec<_>>()
                .join("|");
            let description = format!("P{port} {name} keyboard key");
            variables.push(RetroVariable {
                key: leak_c_string(key),
                value: leak_c_string(format!("{description}; {default}|{rest}")),
            });
        }
    }
    variables.push(RetroVariable {
        key: ptr::null(),
        value: ptr::null(),
    });
    let variables = variables.into_boxed_slice();
    let variables = Box::leak(variables);
    env(
        RETRO_ENVIRONMENT_SET_VARIABLES,
        variables.as_mut_ptr().cast(),
    );

    let mut descriptors = Vec::with_capacity(33);
    for port in 0..=1 {
        for (id, name) in JOYPAD_NAMES {
            descriptors.push(RetroInputDescriptor {
                port,
                device: RETRO_DEVICE_JOYPAD,
                index: 0,
                id,
                description: leak_c_string(format!("P{} {}", port + 1, name)),
            });
        }
    }
    descriptors.push(RetroInputDescriptor {
        port: 0,
        device: 0,
        index: 0,
        id: 0,
        description: ptr::null(),
    });
    let descriptors = Box::leak(descriptors.into_boxed_slice());
    env(
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS,
        descriptors.as_mut_ptr().cast(),
    );
}

unsafe fn environment_call(command: u32, data: *mut c_void) -> bool {
    let Some(environment) = ENVIRONMENT else {
        return false;
    };
    environment(command, data)
}

fn option_value(key: &str) -> Option<String> {
    let key = CString::new(key).ok()?;
    let mut variable = RetroVariable {
        key: key.as_ptr(),
        value: ptr::null(),
    };
    let ok = unsafe {
        environment_call(
            RETRO_ENVIRONMENT_GET_VARIABLE,
            &mut variable as *mut RetroVariable as *mut c_void,
        )
    };
    if !ok || variable.value.is_null() {
        return None;
    }
    unsafe {
        CStr::from_ptr(variable.value)
            .to_str()
            .ok()
            .map(str::to_owned)
    }
}

fn variable_update() -> bool {
    let mut updated = false;
    unsafe {
        let _ = environment_call(
            RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,
            &mut updated as *mut bool as *mut c_void,
        );
    }
    updated
}

unsafe fn system_directory() -> Option<PathBuf> {
    let mut directory: *const c_char = ptr::null();
    if !environment_call(
        RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY,
        &mut directory as *mut *const c_char as *mut c_void,
    ) || directory.is_null()
    {
        return None;
    }
    CStr::from_ptr(directory).to_str().ok().map(PathBuf::from)
}

fn read_system_file(system: Option<&Path>, names: &[&str]) -> Option<Vec<u8>> {
    let system = system?;
    for subdir in [PathBuf::new(), PathBuf::from("libre99")] {
        for name in names {
            let path = system.join(&subdir).join(name);
            if let Ok(bytes) = std::fs::read(path) {
                if !bytes.is_empty() && bytes.len() <= MAX_MEDIA_BYTES {
                    return Some(bytes);
                }
            }
        }
    }
    None
}

fn load_firmware(system: Option<&Path>) -> Firmware {
    Firmware {
        rom: read_system_file(system, &["console-rom.bin", "console.rom", "994aROM.Bin"])
            .unwrap_or_else(|| {
                include_bytes!("../../../original-content/system-roms/rom/console-rom.bin").to_vec()
            }),
        grom: read_system_file(
            system,
            &["console-grom.bin", "console.grom", "994AGROM.Bin"],
        )
        .unwrap_or_else(|| {
            include_bytes!("../../../original-content/system-roms/grom/console-grom.bin").to_vec()
        }),
        dsr: read_system_file(system, &["disk-dsr.bin", "disk.dsr", "Disk.Bin"]).unwrap_or_else(
            || {
                include_bytes!("../../../original-content/system-roms/disk-dsr/disk-dsr.bin")
                    .to_vec()
            },
        ),
    }
}

fn path_string(path: *const c_char) -> Option<String> {
    if path.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(path).to_str().ok().map(str::to_owned) }
}

fn game_data(info: &RetroGameInfo) -> Option<(Option<String>, Vec<u8>)> {
    let path = path_string(info.path);
    if !info.data.is_null() {
        if info.size > MAX_MEDIA_BYTES {
            return None;
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(info.data.cast::<u8>(), info.size) }.to_vec();
        return Some((path, bytes));
    }
    let path_ref = Path::new(path.as_deref()?);
    let bytes = std::fs::read(path_ref).ok()?;
    if bytes.len() > MAX_MEDIA_BYTES {
        return None;
    }
    Some((path, bytes))
}

fn is_disk(path: Option<&str>, bytes: &[u8]) -> bool {
    match path
        .and_then(|path| Path::new(path).extension())
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("dsk") => true,
        Some("ctg") | Some("bin") => false,
        _ => Cartridge::parse(bytes).is_err(),
    }
}

fn disk_label(path: Option<&str>, index: usize) -> String {
    path.and_then(|path| Path::new(path).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("disk {}", index + 1))
}

unsafe fn new_core(info: Option<&RetroGameInfo>) -> Option<Core> {
    let system = system_directory();
    let pixel_format = PIXEL_FORMAT;
    let keyboard_callback = KEYBOARD_CALLBACK_INSTALLED;
    let mut core = Core::new(
        load_firmware(system.as_deref()),
        pixel_format,
        keyboard_callback,
    );
    if let Some(info) = info {
        let no_content = info.path.is_null() && info.data.is_null() && info.size == 0;
        if !no_content {
            let (path, bytes) = game_data(info)?;
            if bytes.is_empty() {
                return None;
            }
            if is_disk(path.as_deref(), &bytes) {
                core.mount_content_disk(DiskImage {
                    key: path.clone(),
                    label: disk_label(path.as_deref(), 0),
                    bytes,
                });
            } else {
                let cartridge = Cartridge::parse(&bytes).ok()?;
                core.machine.mount_cartridge(&cartridge);
                core.machine.set_cart_key(path.as_deref());
                core.machine.reset();
            }
        }
    }
    core.refresh_options(true);
    Some(core)
}

fn file_info_to_disk(info: &RetroGameInfo, index: usize) -> Option<DiskImage> {
    let (path, bytes) = game_data(info)?;
    if bytes.is_empty() || bytes.len() > MAX_MEDIA_BYTES {
        return None;
    }
    Some(DiskImage {
        key: path.clone(),
        label: disk_label(path.as_deref(), index),
        bytes,
    })
}

fn audio_sample_value(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn render_frame(core: &mut Core) {
    core.machine.render(&mut core.framebuffer);
    if core.pixel_format == PixelFormat::Rgb565 {
        for (dst, src) in core.framebuffer_565.iter_mut().zip(&core.framebuffer) {
            *dst = (((*src >> 19) & 0x1F) << 11 | ((*src >> 10) & 0x3F) << 5 | ((*src >> 3) & 0x1F))
                as u16;
        }
    }
}

fn produce_audio(core: &mut Core) {
    core.machine.fill_audio(&mut core.audio_mono);
    for (index, sample) in core.audio_mono.iter().copied().enumerate() {
        let sample = audio_sample_value(sample);
        core.audio_stereo[index * 2] = sample;
        core.audio_stereo[index * 2 + 1] = sample;
    }
}

fn current_core_mut() -> Option<&'static mut Core> {
    unsafe { (*std::ptr::addr_of_mut!(CORE)).as_mut() }
}

#[no_mangle]
pub unsafe extern "C" fn retro_set_environment(environment: Option<RetroEnvironmentFn>) {
    ENVIRONMENT = environment;
    let Some(environment) = environment else {
        return;
    };
    let mut support_no_game = true;
    environment(
        RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME,
        &mut support_no_game as *mut bool as *mut c_void,
    );
    register_core_options(environment);
}

#[no_mangle]
pub unsafe extern "C" fn retro_set_video_refresh(video_refresh: Option<RetroVideoRefreshFn>) {
    VIDEO_REFRESH = video_refresh;
}

#[no_mangle]
pub unsafe extern "C" fn retro_set_audio_sample(audio_sample: Option<RetroAudioSampleFn>) {
    AUDIO_SAMPLE = audio_sample;
}

#[no_mangle]
pub unsafe extern "C" fn retro_set_audio_sample_batch(
    audio_sample_batch: Option<RetroAudioSampleBatchFn>,
) {
    AUDIO_SAMPLE_BATCH = audio_sample_batch;
}

#[no_mangle]
pub unsafe extern "C" fn retro_set_input_poll(input_poll: Option<RetroInputPollFn>) {
    INPUT_POLL = input_poll;
}

#[no_mangle]
pub unsafe extern "C" fn retro_set_input_state(input_state: Option<RetroInputStateFn>) {
    INPUT_STATE = input_state;
}

#[no_mangle]
pub unsafe extern "C" fn retro_init() {
    let mut format = RETRO_PIXEL_FORMAT_XRGB8888;
    if !environment_call(
        RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
        &mut format as *mut u32 as *mut c_void,
    ) {
        format = RETRO_PIXEL_FORMAT_RGB565;
        if !environment_call(
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
            &mut format as *mut u32 as *mut c_void,
        ) {
            format = RETRO_PIXEL_FORMAT_XRGB8888;
        }
    }
    PIXEL_FORMAT = if format == RETRO_PIXEL_FORMAT_RGB565 {
        PixelFormat::Rgb565
    } else {
        PixelFormat::Xrgb8888
    };
    let keyboard_installed = environment_call(
        RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK,
        (&KEYBOARD_CALLBACK as *const RetroKeyboardCallback as *mut RetroKeyboardCallback).cast(),
    );
    KEYBOARD_CALLBACK_INSTALLED = keyboard_installed;
    environment_call(
        RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE,
        (&DISK_CONTROL as *const RetroDiskControlCallback as *mut RetroDiskControlCallback).cast(),
    );
}

#[no_mangle]
pub unsafe extern "C" fn retro_deinit() {
    CORE = None;
    VIDEO_REFRESH = None;
    AUDIO_SAMPLE = None;
    AUDIO_SAMPLE_BATCH = None;
    INPUT_POLL = None;
    INPUT_STATE = None;
    KEYBOARD_CALLBACK_INSTALLED = false;
}

#[no_mangle]
pub extern "C" fn retro_api_version() -> u32 {
    RETRO_API_VERSION
}

#[no_mangle]
pub unsafe extern "C" fn retro_get_system_info(info: *mut RetroSystemInfo) {
    if info.is_null() {
        return;
    }
    *info = RetroSystemInfo {
        library_name: LIBRARY_NAME.as_ptr().cast(),
        library_version: LIBRARY_VERSION.as_ptr().cast(),
        valid_extensions: VALID_EXTENSIONS.as_ptr().cast(),
        need_fullpath: false,
        block_extract: false,
    };
}

#[no_mangle]
pub unsafe extern "C" fn retro_get_system_av_info(info: *mut RetroSystemAvInfo) {
    if info.is_null() {
        return;
    }
    *info = RetroSystemAvInfo {
        geometry: RetroGameGeometry {
            base_width: WIDTH as u32,
            base_height: HEIGHT as u32,
            max_width: WIDTH as u32,
            max_height: HEIGHT as u32,
            aspect_ratio: 4.0 / 3.0,
        },
        timing: RetroSystemTiming {
            fps: 60.0,
            sample_rate: AUDIO_RATE as f64,
        },
    };
}

#[no_mangle]
pub extern "C" fn retro_get_region() -> u32 {
    RETRO_REGION_NTSC
}

#[no_mangle]
pub unsafe extern "C" fn retro_set_controller_port_device(_port: u32, _device: u32) {}

#[no_mangle]
pub unsafe extern "C" fn retro_reset() {
    if let Some(core) = current_core_mut() {
        core.keyboard_events = [None; MAX_KEYCODE];
        core.machine.bus_mut().keyboard.release_all();
        core.machine.reset();
    }
}

#[no_mangle]
pub unsafe extern "C" fn retro_run() {
    if let Some(input_poll) = INPUT_POLL {
        input_poll();
    }
    let (video, audio) = {
        let Some(core) = current_core_mut() else {
            return;
        };
        core.refresh_options(false);
        core.apply_input();
        core.machine.run_frame();
        render_frame(core);
        produce_audio(core);
        let video = match core.pixel_format {
            PixelFormat::Xrgb8888 => (core.framebuffer.as_ptr().cast::<c_void>(), WIDTH * 4),
            PixelFormat::Rgb565 => (core.framebuffer_565.as_ptr().cast::<c_void>(), WIDTH * 2),
        };
        let audio = core.audio_stereo.as_ptr();
        (video, audio)
    };

    if let Some(video_refresh) = VIDEO_REFRESH {
        video_refresh(video.0, WIDTH as u32, HEIGHT as u32, video.1);
    }
    if let Some(audio_batch) = AUDIO_SAMPLE_BATCH {
        audio_batch(audio, AUDIO_FRAMES);
    } else if let Some(audio_sample) = AUDIO_SAMPLE {
        for frame in 0..AUDIO_FRAMES {
            let left = unsafe { *audio.add(frame * 2) };
            let right = unsafe { *audio.add(frame * 2 + 1) };
            audio_sample(left, right);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn retro_serialize_size() -> usize {
    SERIALIZE_SIZE
}

#[no_mangle]
pub unsafe extern "C" fn retro_serialize(data: *mut c_void, size: usize) -> bool {
    if data.is_null() || size < SERIALIZE_SIZE {
        return false;
    }
    let Some(core) = current_core_mut() else {
        return false;
    };
    core.serialize_into(std::slice::from_raw_parts_mut(data.cast::<u8>(), size))
}

#[no_mangle]
pub unsafe extern "C" fn retro_unserialize(data: *const c_void, size: usize) -> bool {
    if data.is_null() || size > SERIALIZE_SIZE {
        return false;
    }
    let Some(core) = current_core_mut() else {
        return false;
    };
    core.unserialize(std::slice::from_raw_parts(data.cast::<u8>(), size))
}

#[no_mangle]
pub unsafe extern "C" fn retro_load_game(info: *const RetroGameInfo) -> bool {
    let info = info.as_ref();
    let Some(core) = new_core(info) else {
        return false;
    };
    CORE = Some(core);
    true
}

#[no_mangle]
pub unsafe extern "C" fn retro_load_game_special(
    _game_type: u32,
    _info: *const RetroGameInfo,
    _num_info: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn retro_unload_game() {
    CORE = None;
}

#[no_mangle]
pub unsafe extern "C" fn retro_cheat_reset() {}

#[no_mangle]
pub unsafe extern "C" fn retro_cheat_set(_index: u32, _enabled: bool, _code: *const c_char) {}

#[no_mangle]
pub unsafe extern "C" fn retro_get_memory_data(_id: u32) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn retro_get_memory_size(_id: u32) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn retro_keyboard_event(
    down: bool,
    keycode: u32,
    character: u32,
    modifiers: u16,
) {
    if let Some(core) = current_core_mut() {
        if let Some(slot) = core.keyboard_events.get_mut(keycode as usize) {
            *slot = down.then_some(KeyboardEventState {
                character,
                modifiers,
            });
        }
    }
}

unsafe extern "C" fn disk_set_eject_state(ejected: bool) -> bool {
    current_core_mut().is_some_and(|core| core.set_eject_state(ejected))
}

unsafe extern "C" fn disk_get_eject_state() -> bool {
    current_core_mut().is_none_or(|core| core.disk_ejected)
}

unsafe extern "C" fn disk_get_image_index() -> u32 {
    current_core_mut().map_or(0, |core| core.disk_index as u32)
}

unsafe extern "C" fn disk_set_image_index(index: u32) -> bool {
    let Some(core) = current_core_mut() else {
        return false;
    };
    if !core.disk_ejected || index as usize >= core.disk_images.len() {
        return false;
    }
    core.disk_index = index as usize;
    true
}

unsafe extern "C" fn disk_get_num_images() -> u32 {
    current_core_mut().map_or(0, |core| core.disk_images.len() as u32)
}

unsafe extern "C" fn disk_replace_image_index(index: u32, info: *const RetroGameInfo) -> bool {
    let Some(core) = current_core_mut() else {
        return false;
    };
    let image = if info.is_null() {
        None
    } else {
        file_info_to_disk(unsafe { &*info }, index as usize)
    };
    core.replace_disk(index as usize, image)
}

unsafe extern "C" fn disk_add_image_index() -> bool {
    current_core_mut().is_some_and(Core::add_disk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static VIDEO_FRAMES: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn test_environment(command: u32, data: *mut c_void) -> bool {
        match command {
            RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY => {
                if data.is_null() {
                    return false;
                }
                *(data as *mut *const c_char) = ptr::null();
                true
            }
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
                if data.is_null() {
                    return false;
                }
                matches!(
                    *(data as *const u32),
                    RETRO_PIXEL_FORMAT_XRGB8888 | RETRO_PIXEL_FORMAT_RGB565
                )
            }
            RETRO_ENVIRONMENT_GET_VARIABLE => false,
            RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => {
                if data.is_null() {
                    return false;
                }
                *(data as *mut bool) = false;
                true
            }
            RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME
            | RETRO_ENVIRONMENT_SET_VARIABLES
            | RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS
            | RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK
            | RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE => true,
            _ => false,
        }
    }

    unsafe extern "C" fn test_video(
        _data: *const c_void,
        _width: u32,
        _height: u32,
        _pitch: usize,
    ) {
        VIDEO_FRAMES.fetch_add(1, Ordering::Relaxed);
    }

    unsafe extern "C" fn test_audio(_left: i16, _right: i16) {}
    unsafe extern "C" fn test_audio_batch(_data: *const i16, frames: usize) -> usize {
        frames
    }
    unsafe extern "C" fn test_input_poll() {}
    unsafe extern "C" fn test_input_state(_port: u32, _device: u32, _index: u32, _id: u32) -> i16 {
        0
    }

    #[test]
    fn controller_defaults_cover_both_ti_joysticks_and_keyboard_keys() {
        let config = InputConfig::default();
        assert_eq!(
            config.p1[RETRO_DEVICE_ID_JOYPAD_UP as usize],
            Some(TiKey::Joy1Up)
        );
        assert_eq!(
            config.p1[RETRO_DEVICE_ID_JOYPAD_A as usize],
            Some(TiKey::Joy1Fire)
        );
        assert_eq!(
            config.p2[RETRO_DEVICE_ID_JOYPAD_RIGHT as usize],
            Some(TiKey::Joy2Right)
        );
        assert_eq!(
            config.p1[RETRO_DEVICE_ID_JOYPAD_B as usize],
            Some(TiKey::Space)
        );
    }

    #[test]
    fn keyboard_character_map_uses_ti_modifiers() {
        assert_eq!(
            char_to_ti_press('@'),
            [Some(TiKey::Shift), Some(TiKey::Num2)]
        );
        assert_eq!(char_to_ti_press('"'), [Some(TiKey::Fctn), Some(TiKey::P)]);
        assert_eq!(
            resolve_keyboard(RETROK_BACKSPACE, 0, 0),
            [Some(TiKey::Fctn), Some(TiKey::S)]
        );
    }

    #[test]
    fn metadata_round_trips_disk_playlist() {
        let mut writer = MetadataWriter::default();
        writer.u32(0);
        writer.u8(1);
        writer.u32(1);
        writer.optional_string(Some("game.dsk"));
        writer.string("game.dsk");
        writer.blob(&[1, 2, 3]);
        let (index, ejected, images) = MetadataReader::new(&writer.bytes).read_all().unwrap();
        assert_eq!(index, 0);
        assert!(ejected);
        assert_eq!(images[0].key.as_deref(), Some("game.dsk"));
        assert_eq!(images[0].bytes, vec![1, 2, 3]);
    }

    #[test]
    fn libretro_lifecycle_submits_video() {
        VIDEO_FRAMES.store(0, Ordering::Relaxed);
        unsafe {
            retro_set_environment(Some(test_environment));
            retro_set_video_refresh(Some(test_video));
            retro_set_audio_sample(Some(test_audio));
            retro_set_audio_sample_batch(Some(test_audio_batch));
            retro_set_input_poll(Some(test_input_poll));
            retro_set_input_state(Some(test_input_state));
            retro_init();
            assert!(retro_load_game(ptr::null()));
            retro_run();
            assert_eq!(VIDEO_FRAMES.load(Ordering::Relaxed), 1);
            retro_unload_game();
            retro_deinit();
        }
    }

    #[test]
    fn bare_core_state_wrapper_round_trips() {
        let mut core = Core::new(load_firmware(None), PixelFormat::Xrgb8888, false);
        let mut state = vec![0u8; SERIALIZE_SIZE];
        assert!(core.serialize_into(&mut state));
        assert!(core.unserialize(&state));
    }

    #[test]
    fn cartridge_and_disk_detection_use_extension_then_content() {
        assert!(is_disk(Some("disk.DSK"), b"anything"));
        assert!(!is_disk(Some("game.ctg"), b"not-a-cartridge"));
        assert!(is_disk(None, b"not-a-cartridge"));
    }

    #[test]
    fn loading_disk_content_mounts_dsk1_for_core_lifetime() {
        let bytes = vec![0; 4096];
        let path = CString::new("content.dsk").unwrap();
        let info = RetroGameInfo {
            path: path.as_ptr(),
            data: bytes.as_ptr().cast(),
            size: bytes.len(),
            meta: ptr::null(),
        };
        let core = unsafe { new_core(Some(&info)) }.expect("disk content should load");

        assert_eq!(core.disk_images.len(), 1);
        assert!(!core.disk_ejected);
        assert_eq!(
            core.machine.bus().disk.drive_image(0),
            Some(bytes.as_slice())
        );
    }
}
