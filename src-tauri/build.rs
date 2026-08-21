fn main() {
    let brand_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/brand.json");
    let brand = std::fs::read_to_string(&brand_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", brand_path.display()));
    let v: serde_json::Value =
        serde_json::from_str(&brand).expect("parse src/brand.json");
    let name = v["productName"]
        .as_str()
        .unwrap_or("App");
    println!("cargo:rustc-env=APP_PRODUCT_NAME={name}");
    tauri_build::build()
}
