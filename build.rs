use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let icon_source_dir = "icons";
    let icon_dest_dir = Path::new(&out_dir).join("../../../icons");

    fs::create_dir_all(&icon_dest_dir).expect("Failed to create destination directory");

    let entries = fs::read_dir(icon_source_dir).expect("Failed to read source directory");

    for entry in entries {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("svg") {
            let file_name = path.file_name().unwrap();
            let dest_path = icon_dest_dir.join(file_name);

            fs::copy(&path, &dest_path).expect(&format!("Failed to copy {:?}", file_name));
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    println!("cargo:rerun-if-changed={}", icon_source_dir);
}
