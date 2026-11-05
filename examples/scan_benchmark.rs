//! Synthetic single-directory scan benchmark. Fixture creation is timed separately.
use anyhow::Result;
use filespring::{Engine, browser};
use serde_json::json;
use std::{
    fs,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

fn main() -> Result<()> {
    let counts = std::env::args()
        .skip(1)
        .map(|x| x.parse::<usize>())
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let counts = if counts.is_empty() {
        vec![1_000, 10_000, 100_000]
    } else {
        counts
    };
    let mut results = vec![];
    for count in counts {
        let fixture = tempfile::tempdir()?;
        let root = fs::canonicalize(fixture.path())?;
        let dir = root.join("files");
        fs::create_dir(&dir)?;
        let start = Instant::now();
        for i in 0..count {
            fs::write(
                dir.join(format!("fixture-{i:08}.txt")),
                b"generated benchmark fixture\n",
            )?;
        }
        let generation_ms = start.elapsed().as_secs_f64() * 1000.0;
        let engine = Engine::open(&root.join("state"), std::slice::from_ref(&root))?;
        let mut runs = vec![];
        for _ in 0..3 {
            let start = Instant::now();
            let mut observed = 0;
            browser::scan(
                engine.scope(),
                &dir,
                &Arc::new(AtomicBool::new(false)),
                |b| observed += b.len(),
            )?;
            assert_eq!(observed, count);
            runs.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        results.push(json!({"files":count,"fixture_generation_ms":generation_ms,"scan_ms":runs,"bytes_per_file":28,"cache":"warm after generation; not a cold HDD test"}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"build":"release if invoked with --release","dataset":"synthetic regular files in an isolated temporary directory","results":results})
        )?
    );
    Ok(())
}
