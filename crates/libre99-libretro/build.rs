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

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=libre99_libretro.info");

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo always sets CARGO_MANIFEST_DIR"),
    );
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo always sets OUT_DIR"));
    let profile_dir = out_dir
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .expect("OUT_DIR must be target/<profile>/build/<package>/out");
    let source = manifest_dir.join("libre99_libretro.info");
    let destination = profile_dir.join("libre99_libretro.info");

    fs::copy(&source, &destination).unwrap_or_else(|error| {
        panic!(
            "could not copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    });
}
