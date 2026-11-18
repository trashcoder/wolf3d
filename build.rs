use std::{env, fs, path::PathBuf};

fn main() {
    extract_signon_image();
    println!("cargo:rerun-if-env-changed=SDL2_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SDL2_FRAMEWORK_DIR");

    if let Some(path) = env::var_os("SDL2_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", path.to_string_lossy());
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        if let Some(path) = env::var_os("SDL2_FRAMEWORK_DIR") {
            println!(
                "cargo:rustc-link-search=framework={}",
                path.to_string_lossy()
            );
            println!("cargo:rustc-link-lib=framework=SDL2");
        } else {
            println!("cargo:rustc-link-lib=dylib=SDL2");
        }
    }

    if matches!(target_os.as_str(), "windows" | "macos")
        && env::var_os("SDL2_LIB_DIR").is_none()
        && env::var_os("SDL2_FRAMEWORK_DIR").is_none()
    {
        println!(
            "cargo:warning=SDL2_LIB_DIR is unset; SDL2 must be available in the target linker's default search path"
        );
    }
}

fn extract_signon_image() {
    const IMAGE_SIZE: usize = 320 * 200;
    const IMAGE_SEGMENT: u8 = 1;
    let source_path = PathBuf::from("WOLFSRC/OBJ/SIGNON.OBJ");
    println!("cargo:rerun-if-changed={}", source_path.display());
    let object = fs::read(&source_path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", source_path.display()));
    let mut image = vec![0; IMAGE_SIZE];
    let mut written = vec![false; IMAGE_SIZE];
    let mut cursor = 0;

    while cursor < object.len() {
        assert!(cursor + 3 <= object.len(), "truncated OMF record header");
        let record_type = object[cursor];
        let record_length = u16::from_le_bytes([object[cursor + 1], object[cursor + 2]]) as usize;
        let record_end = cursor
            .checked_add(3 + record_length)
            .filter(|&end| end <= object.len())
            .expect("truncated OMF record");

        if matches!(record_type, 0xa0 | 0xa1) {
            // LEDATA carries a segment index, a 16- or 32-bit offset, bytes,
            // and the final OMF checksum. SIGNON.OBJ stores signon.bin in
            // segment 1, as confirmed by its linker map.
            let payload_start = cursor + 3;
            let payload_end = record_end - 1;
            let payload = &object[payload_start..payload_end];
            let offset_width = if record_type == 0xa0 { 2 } else { 4 };
            let header_width = 1 + offset_width;
            assert!(payload.len() >= header_width, "short OMF LEDATA record");
            if payload[0] == IMAGE_SEGMENT {
                let image_offset = payload[1..header_width]
                    .iter()
                    .enumerate()
                    .fold(0usize, |offset, (shift, byte)| {
                        offset | (usize::from(*byte) << (shift * 8))
                    });
                let data = &payload[header_width..];
                let image_end = image_offset
                    .checked_add(data.len())
                    .filter(|&end| end <= IMAGE_SIZE)
                    .expect("SIGNON.OBJ image data exceeds 320x200 pixels");
                assert!(
                    written[image_offset..image_end].iter().all(|byte| !byte),
                    "overlapping signon image data in SIGNON.OBJ"
                );
                image[image_offset..image_end].copy_from_slice(data);
                written[image_offset..image_end].fill(true);
            }
        }
        cursor = record_end;
    }

    assert!(
        written.iter().all(|byte| *byte),
        "SIGNON.OBJ did not provide a complete 320x200 signon image"
    );
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    fs::write(out_dir.join("signon.bin"), image).expect("could not write extracted signon image");
}
