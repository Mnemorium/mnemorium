use std::fs;
use std::path::Path;

use anyhow::Result;

use mnemorium::infrastructure::inbound::rest::ApiDoc;
use utoipa::OpenApi as _;

fn main() -> Result<()> {
    let value = serde_json::to_value(ApiDoc::openapi())?;
    let spec = serde_json::to_string_pretty(&value)?;

    let output_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/development/api");
    let output_path = output_dir.join("openapi.json");

    fs::create_dir_all(&output_dir)?;
    fs::write(&output_path, spec)?;

    tracing::info!("wrote OpenAPI spec to {}", output_path.display());

    Ok(())
}
