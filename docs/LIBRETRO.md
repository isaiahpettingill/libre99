# Libre99 libretro core

`libre99-libretro` is a separate Rust `cdylib` adapter for RetroArch and other
libretro frontends. It links the existing `libre99-core`; it does not change the
emulator or the desktop app.

The adapter is MIT-licensed by Isaiah Pettingill. The Libre99 emulator core and
clean-room firmware remain Joel Odom's work under their existing license. The
adapter's Rust source carries its own MIT notice; it does not relicense the
linked core or firmware.

## Build and install

Build the dynamic library from the workspace root:

```bash
cargo build --release -p libre99-libretro
```

Cargo writes the core and its metadata to `target/release`:

- Windows: `libre99_libretro.dll` + `libre99_libretro.info`
- Linux: `liblibre99_libretro.so` + `libre99_libretro.info`
- macOS: `liblibre99_libretro.dylib` + `libre99_libretro.info`

The build script copies the `.info` file beside the library in both `target/debug`
and `target/release`. Copy both files to the frontend's cores directory. The
exact library extension expected by a frontend can differ; keep the library name
`libre99_libretro` when possible.

The core reports ABI 1, NTSC timing, 256×192 video, 60 Hz, and 44.1 kHz stereo
audio. It supports booting without content, so loading the core by itself opens
the bare console.

## Content

Load one of these files as core content:

| File | Result |
|---|---|
| `.ctg` | TI-99/4A cartridge container, cold-booted in the cartridge port |
| raw `.bin` | Raw CPU-ROM cartridge dump, cold-booted in the cartridge port |
| `.dsk` | Raw TI sector image mounted in DSK1 |

The frontend owns content selection. Put cartridges and disks in any directory
you use for game content, then load them through the frontend's normal content
browser. The core copies frontend-provided buffers, so it does not depend on
those buffers remaining valid after `retro_load_game` returns.

A `.dsk` loaded directly as core content is copied into memory and inserted in
DSK1 for the duration of that core session. Unloading the content removes the
in-memory disk; the source file is never modified. A `.dsk` is a disk image, not
a self-launching program. Loading one by itself boots the bare console; the
console has no generic disk browser or automatic file launcher. Disk software
normally needs a cartridge or console firmware program to read the directory
and load files.

For cartridge-based disk software in RetroArch:

1. Load the `.ctg` or raw `.bin` cartridge as the core content.
2. Open **Quick Menu → Disk Control**.
3. Use **Load New Disk** (the exact label can vary by frontend) to add the
   `.dsk`, then select its disk index and insert it.
4. Start the cartridge's disk option or program. The cartridge reads DSK1;
   there is no separate host-side file browser.

For TI BASIC disk programs, use console ROM/GROM and a disk DSR that you are
allowed to use. The built-in clean-room firmware does not implement the
console-resident TI BASIC interpreter. Once authentic firmware is loaded, a
normal BASIC workflow is:

```text
OLD DSK1.PROGRAM
RUN
```

Replace `PROGRAM` with the filename on the disk. A cartridge may provide its
own menu or commands instead of `OLD` and `RUN`.

For disk content, the libretro disk-control interface exposes a playlist for
DSK1. Frontends can eject the current image, add or replace images, and select
a new image. DSK2 and DSK3 remain available to TI software through the emulated
controller but are not represented as separate libretro playlist entries.

Disk writes stay in memory. The core never writes back to the source `.dsk`
file. Save states include the modified image and the libretro disk playlist.
Use the frontend's content or disk management tools if you need to replace a
host file.

## Custom firmware

The clean-room console ROM, console GROM, and disk DSR are built into the core.
A libretro frontend normally supplies its system directory through the
`GET_SYSTEM_DIRECTORY` environment callback. The core checks that directory,
then `system/libre99`, for these files:

| Component | Names checked |
|---|---|
| Console ROM | `console-rom.bin`, `console.rom`, `994aROM.Bin` |
| Console GROM | `console-grom.bin`, `console.grom`, `994AGROM.Bin` |
| Disk DSR | `disk-dsr.bin`, `disk.dsr`, `Disk.Bin` |

Each component is selected independently. If a file is absent, the built-in
clean-room component is used. Authentic TI firmware is not distributed with
this repository; only use images you are allowed to use.

This also makes firmware testing practical: place replacement images in the
frontend system folder, restart the core, and load content normally.

## Input

The core polls two libretro joypad ports:

- port 1 drives the TI joystick 1 matrix;
- port 2 drives the TI joystick 2 matrix.

The default controller mapping is:

| Libretro input | Port 1 | Port 2 |
|---|---|---|
| D-pad | joystick 1 directions | joystick 2 directions |
| A | joystick 1 fire | joystick 2 fire |
| B | TI `SPACE` | TI `SPACE` |
| Y | TI `ENTER` | TI `ENTER` |
| X | TI `SHIFT` | TI `SHIFT` |
| L | TI `FCTN` | TI `FCTN` |
| R | TI `CTRL` | TI `CTRL` |
| Start | TI `CTRL` | TI `CTRL` |
| Select | TI `FCTN` | TI `FCTN` |

Every joypad button can be assigned to any named TI matrix key through the
core options. The option keys are `libre99_p1_<button>` and
`libre99_p2_<button>`, for example:

```text
libre99_p1_a = space
libre99_p1_b = enter
libre99_p1_up = w
libre99_p1_down = s
libre99_p1_left = a
libre99_p1_right = d
```

The available values include letters, numbers, `space`, `enter`, `fctn`,
`shift`, `ctrl`, punctuation names, `joy1_*`, `joy2_*`, and `none`. The exact
option UI is provided by the frontend.

Keyboard input uses the libretro keyboard callback when available and falls
back to `RETRO_DEVICE_KEYBOARD` polling otherwise. It supports letters,
numbers, TI modifiers, arrows as joystick 1, editing keys, and printable
characters with synthesized TI `SHIFT`/`FCTN` combinations.

Mouse input is also accepted. Relative movement drives joystick 1 for the
current frame; left, right, and middle buttons map to controller A, B, and X;
mouse wheel up/down map to Y/B.

## Save states

The core exposes the normal libretro serialize/unserialize functions. The
adapter wraps the machine's portable state with its disk-playlist metadata and
uses a fixed 16 MiB serialization buffer so the reported state size does not
change when disks are swapped. The machine state still carries the selected
firmware, cartridge, RAM, VRAM, keyboard matrix, PSG, and in-memory disks.

The current adapter accepts media up to 8 MiB. This bounds malformed content and
keeps state sizes predictable for frontends.

## Scope

The adapter currently exposes the hardware already present in `libre99-core`:
video, PSG audio, keyboard/joysticks, mouse-to-joystick input, cartridges,
disks, custom console firmware, and save states. Speech synthesis and cassette
hardware are not emulated by the underlying machine yet.
