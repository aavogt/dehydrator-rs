// Necessary because of this issue: https://github.com/rust-lang/cargo/issues/9641

use serde::{Deserialize, Serialize};
use typescript_type_def::{write_definition_file,DefinitionFileOptions};
include!("src/json.rs");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    embuild::build::CfgArgs::output_propagated("ESP_IDF")?;
    embuild::build::LinkArgs::output_propagated("ESP_IDF")?;
    // now call write_definition_file::<_, Foo>(&mut buf, options)
    // on a writer that outputs to OUT_DIR/json.ts
    // first create a string representing OUT_DIR/json.ts
    // let out_dir = std::env::var("OUT_DIR")?;
    let out_dir = "www";
    let json_path = std::path::Path::new(&out_dir).join("json.ts");
    let mut file = std::fs::File::create(json_path)?;
    let options = DefinitionFileOptions::default();
    write_definition_file::<_, API>(&mut file, options)?;
    Ok(())
}
