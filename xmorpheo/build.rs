// SPDX-License-Identifier: GPL-2.0-or-later

//! Builds the BPF program into a skeleton, and the BPF-side math for the
//! host, so tests can compare it with the Rust reference.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use libbpf_cargo::SkeletonBuilder;

const BPF_DIR: &str = "../bpf";

/// Writes vmlinux.h for the running kernel into `out`, unless
/// `XMORPHEO_VMLINUX_H` points to a directory that already has one.
fn vmlinux_dir(out: &Path) -> PathBuf {
    println!("cargo:rerun-if-env-changed=XMORPHEO_VMLINUX_H");
    if let Some(dir) = env::var_os("XMORPHEO_VMLINUX_H") {
        return PathBuf::from(dir);
    }
    let header = out.join("vmlinux.h");
    let output = Command::new("bpftool")
        .args([
            "btf",
            "dump",
            "file",
            "/sys/kernel/btf/vmlinux",
            "format",
            "c",
        ])
        .output()
        .expect("bpftool must be installed to generate vmlinux.h");
    assert!(
        output.status.success(),
        "bpftool failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(&header, output.stdout).unwrap();
    out.to_path_buf()
}

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let vmlinux = vmlinux_dir(&out);
    let src = Path::new(BPF_DIR).join("xmorpheo.bpf.c");
    let header = Path::new(BPF_DIR).join("xmorpheo.h");
    let host = Path::new(BPF_DIR).join("xmorpheo_host.c");

    SkeletonBuilder::new()
        .source(&src)
        .clang_args([
            "-I".as_ref(),
            vmlinux.as_os_str(),
            "-I".as_ref(),
            BPF_DIR.as_ref(),
            // Kernels built with -fms-extensions embed tagged structs, which
            // bpftool dumps as bare declarations.
            "-fms-extensions".as_ref(),
            "-Wno-microsoft-anon-tag".as_ref(),
            "-Wno-missing-declarations".as_ref(),
        ])
        .build_and_generate(out.join("xmorpheo.skel.rs"))
        .expect("building the BPF program failed");

    cc::Build::new()
        .file(&host)
        .include(BPF_DIR)
        .warnings(true)
        .compile("xmorpheo_host");

    for path in [&src, &header, &host] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
