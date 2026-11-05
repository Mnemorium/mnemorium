//! The server launcher.
//!
//! The composition root lives in the library (`src/lib/app.rs`, `STY-RUST-067`);
//! this binary only launches it.

use mnemorium::app;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
