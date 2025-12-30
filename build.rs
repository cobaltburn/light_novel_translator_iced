use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let source_dir = "icons";
    let dest_dir = Path::new(&out_dir).join("../../../icons");

    fs::create_dir_all(&dest_dir).expect("Failed to create destination directory");

    let entries = fs::read_dir(source_dir).expect("Failed to read source directory");

    for entry in entries {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("svg") {
            let file_name = path.file_name().unwrap();
            let dest_path = dest_dir.join(file_name);

            fs::copy(&path, &dest_path).expect(&format!("Failed to copy {:?}", file_name));
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    println!("cargo:rerun-if-changed={}", source_dir);
}
