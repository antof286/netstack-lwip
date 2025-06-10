// build.rs
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

/// -------- Apple helpers ---------------------------------------------------
fn sdk_include_path_for(sdk: &str) -> String {
    let output = Command::new("xcrun")
        .arg("--sdk")
        .arg(sdk)
        .arg("--show-sdk-path")
        .output()
        .expect("failed to execute xcrun");

    Path::new(String::from_utf8_lossy(&output.stdout).trim())
        .join("usr/include")
        .to_str()
        .expect("invalid include path")
        .to_owned()
}

fn apple_sdk_include_path() -> Option<String> {
    let os = env::var("CARGO_CFG_TARGET_OS").ok()?;
    let target = env::var("TARGET").ok()?;

    match os.as_str() {
        "ios" => {
            if target.ends_with("-ios-sim")
                || target == "x86_64-apple-ios"
            {
                Some(sdk_include_path_for("iphonesimulator"))
            } else {
                Some(sdk_include_path_for("iphoneos"))
            }
        }
        "macos" => Some(sdk_include_path_for("macosx")),
        _ => None,
    }
}

/// -------- Android helpers --------------------------------------------------
fn android_target_triple() -> Option<String> {
    let os = env::var("CARGO_CFG_TARGET_OS").ok()?;
    if os == "android" {
        Some(env::var("TARGET").expect("TARGET unset"))
    } else {
        None
    }
}

/// API level to use when none is supplied.
const DEFAULT_API: &str = "21";

fn android_api_level() -> String {
    env::var("ANDROID_API")
        .or_else(|_| env::var("ANDROID_API_LEVEL"))
        .unwrap_or_else(|_| DEFAULT_API.into())
}

/// -------- Common build steps ----------------------------------------------
fn compile_lwip() {
    let mut build = cc::Build::new();
    build
        .file("src/lwip/core/init.c")
        .file("src/lwip/core/def.c")
        .file("src/lwip/core/inet_chksum.c")
        .file("src/lwip/core/ip.c")
        .file("src/lwip/core/mem.c")
        .file("src/lwip/core/memp.c")
        .file("src/lwip/core/netif.c")
        .file("src/lwip/core/pbuf.c")
        .file("src/lwip/core/raw.c")
        .file("src/lwip/core/tcp.c")
        .file("src/lwip/core/tcp_in.c")
        .file("src/lwip/core/tcp_out.c")
        .file("src/lwip/core/timeouts.c")
        .file("src/lwip/core/udp.c")
        .file("src/lwip/core/ipv4/icmp.c")
        .file("src/lwip/core/ipv4/ip4_frag.c")
        .file("src/lwip/core/ipv4/ip4.c")
        .file("src/lwip/core/ipv4/ip4_addr.c")
        .file("src/lwip/core/ipv6/icmp6.c")
        .file("src/lwip/core/ipv6/ip6.c")
        .file("src/lwip/core/ipv6/ip6_addr.c")
        .file("src/lwip/core/ipv6/ip6_frag.c")
        .file("src/lwip/core/ipv6/nd6.c")
        .file("src/lwip/custom/sys_arch.c")
        .include("src/lwip/custom")
        .include("src/lwip/include")
        .warnings(false)
        .flag_if_supported("-Wno-everything");

    // Platform-specific tweaks
    if let Some(path) = apple_sdk_include_path() {
        build.include(path);
    }
    if android_target_triple().is_some() {
        // Let C know which API level we’re building for
        build.define(
            "__ANDROID_API__",
            Some(android_api_level().as_str()),
        );
    }

    build.debug(true);
    build.compile("liblwip.a");
}

fn generate_lwip_bindings() {
    println!("cargo:rustc-link-lib=static=lwip");
    println!("cargo:include=src/lwip/include");

    let arch   = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let os     = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target = env::var("TARGET").unwrap();

    let mut builder = bindgen::Builder::default()
        .header("src/lwip/wrapper.h")
        .size_t_is_usize(false)
        .clang_arg("-I./src/lwip/include")
        .clang_arg("-I./src/lwip/custom")
        .clang_arg("-Wno-everything")
        .layout_tests(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks));

    // Tell clang which target we are generating for
    match os.as_str() {
        "ios" if arch == "aarch64" => {
            // https://github.com/rust-lang/rust-bindgen/issues/1211
            builder = builder.clang_arg("--target=arm64-apple-ios");
        }
        "android" => {
            builder = builder.clang_arg(format!("--target={}", target));

            // If code completion / IDE support is desired you can also pass
            // the sysroot explicitly, but Clang usually finds it:
            // if let Ok(ndk) = env::var("ANDROID_NDK_HOME") {
            //     let sysroot = format!("{ndk}/toolchains/llvm/prebuilt/{host}/sysroot");
            //     builder = builder.clang_arg(format!("--sysroot={}", sysroot));
            // }
        }
        _ => {}
    }

    if let Some(path) = apple_sdk_include_path() {
        builder = builder.clang_arg(format!("-I{}", path));
    }

    let bindings = builder
        .generate()
        .expect("Unable to generate bindings");

    let out = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    bindings
        .write_to_file(out.join("src/bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn main() {
    // Re-run if any lwIP source changes
    println!("cargo:rerun-if-changed=src/lwip");
    generate_lwip_bindings();
    compile_lwip();
}
