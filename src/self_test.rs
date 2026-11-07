use anyhow::{Result, ensure};
use filespring::{Action, Engine, PlanState, Request};
use serde_json::json;
use std::{fs, path::PathBuf};

/// A portable diagnostic with no Python/PowerShell dependency. Only a freshly
/// created fixture can be modified. Keep its report and samples for inspection.
pub fn run(fixture: Option<PathBuf>) -> Result<()> {
    let path = fixture.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("FileSpring-verify-{}", uuid::Uuid::new_v4()))
    });
    fs::create_dir(&path)?;
    let root = fs::canonicalize(path)?;
    let mut checks = vec![];
    let result = (|| -> Result<()> {
        fs::create_dir(root.join("Inbox"))?;
        fs::create_dir(root.join("Library"))?;
        let source = root.join("Inbox/中文 sample & space.txt");
        let destination = root.join("Library/中文 sample & space.txt");
        let contents = b"FileSpring isolated verification fixture\n";
        fs::write(&source, contents)?;
        let mut engine = Engine::open(&root.join("state"), std::slice::from_ref(&root))?;
        for action in [Action::Copy, Action::Move] {
            let plan = engine.prepare(vec![Request {
                action,
                source: source.clone(),
                destination: destination.clone(),
            }])?;
            ensure!(
                plan.executable() && source.exists() && !destination.exists(),
                "Preview changed files"
            );
            checks.push(format!("{action:?}: preview leaves files unchanged"));
            ensure!(
                engine.execute(&plan.id)?.state == PlanState::Completed,
                "Incomplete execution"
            );
            ensure!(
                fs::read(&destination)? == contents,
                "Destination content mismatch"
            );
            ensure!(
                source.exists() == (action == Action::Copy),
                "Source state mismatch"
            );
            checks.push(format!("{action:?}: verified execution"));
            ensure!(
                engine.rollback(&plan.id)?.state == PlanState::RolledBack,
                "Incomplete undo"
            );
            ensure!(
                fs::read(&source)? == contents && !destination.exists(),
                "Undo content mismatch"
            );
            checks.push(format!("{action:?}: confirmed undo restores fixture"));
        }
        fs::write(&destination, b"unrelated generated conflict sample")?;
        let conflict = engine.prepare(vec![Request {
            action: Action::Move,
            source: source.clone(),
            destination: destination.clone(),
        }])?;
        ensure!(
            !conflict.executable() && engine.execute(&conflict.id).is_err(),
            "Conflict was not blocked"
        );
        ensure!(
            fs::read(&destination)? == b"unrelated generated conflict sample" && source.exists(),
            "Conflict changed files"
        );
        checks.push("Existing destination: preserved".into());
        ensure!(
            Engine::open(&root.join("state"), std::slice::from_ref(&root)).is_err(),
            "Journal lock not exclusive"
        );
        checks.push("Concurrent journal: blocked".into());
        drop(engine);
        let engine = Engine::open(&root.join("state"), std::slice::from_ref(&root))?;
        ensure!(engine.history()?.len() == 3, "Persistent history missing");
        checks.push("Journal reopen: persistent history present".into());
        Ok(())
    })();
    let report = json!({
        "result": if result.is_ok() { "passed" } else { "failed" },
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "fixture": root,
        "checks": checks,
        "error": result.as_ref().err().map(|error| format!("{error:#}")),
        "scope": "New generated fixture only. This does not certify GUI, Windows 10/11 compatibility, power-loss recovery, or cross-volume operation."
    });
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(root.join("verification.json"), &json)?;
    println!("{json}");
    result
}
