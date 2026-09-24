//! Runs the golden fixtures (fixtures/*.json) against a real or mock processor
//! and prints a pass/fail table. Not a CI gate for real models.
//!
//!   cargo run -p thoughtrouter-core --example eval -- --mock
//!   OPENROUTER_API_KEY=… cargo run -p thoughtrouter-core --example eval -- \
//!       --analyzer vendor/model [--embedder vendor/embedding-model] [--zdr]

use std::path::PathBuf;
use std::sync::Arc;

use thoughtrouter_core::eval::{load_fixtures, run_fixture};
use thoughtrouter_core::processor::mock::{MockAnalyzer, MockEmbedder};
use thoughtrouter_core::processor::openrouter::{OpenRouter, OpenRouterConfig};
use thoughtrouter_core::processor::{Embedder, Processors};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let processors = if args.iter().any(|a| a == "--mock") {
        Processors {
            analyzer: Arc::new(MockAnalyzer),
            embedder: Some(Arc::new(MockEmbedder)),
        }
    } else {
        let key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| anyhow::anyhow!("set OPENROUTER_API_KEY (or pass --mock)"))?;
        let analyzer = arg(&args, "--analyzer")
            .ok_or_else(|| anyhow::anyhow!("pass --analyzer <model id>"))?;
        let embedder_model = arg(&args, "--embedder").unwrap_or_default();
        let or = Arc::new(OpenRouter::new(OpenRouterConfig::new(
            key,
            analyzer,
            embedder_model.clone(),
            args.iter().any(|a| a == "--zdr"),
        ))?);
        let embedder: Option<Arc<dyn Embedder>> = if embedder_model.is_empty() {
            None
        } else {
            Some(or.clone())
        };
        Processors {
            analyzer: or,
            embedder,
        }
    };
    let dir = arg(&args, "--fixtures")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"));
    let fixtures = load_fixtures(&dir)?;
    let mut passed = 0;
    for f in &fixtures {
        let r = run_fixture(f, &processors).await?;
        println!(
            "{} {:<28} {}",
            if r.passed() { "PASS" } else { "FAIL" },
            r.id,
            f.description
        );
        for line in &r.produced {
            println!("       {line}");
        }
        for fail in &r.failures {
            println!("     ✗ {fail}");
        }
        if r.passed() {
            passed += 1;
        }
    }
    println!("\n{passed}/{} fixtures passed", fixtures.len());
    if passed < fixtures.len() {
        std::process::exit(1);
    }
    Ok(())
}
