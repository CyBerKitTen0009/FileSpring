use crate::{
    filesystem::{self, Scope},
    model::*,
};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use rusqlite::{Connection, params};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Engine {
    db: Connection,
    scope: Scope,
    _lock: File,
}

impl Engine {
    pub fn open(state: &Path, roots: &[PathBuf]) -> Result<Self> {
        fs::create_dir_all(state)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(state.join("engine.lock"))?;
        lock.try_lock_exclusive()
            .context("Another FileSpring process is using this journal")?;
        let db = Connection::open(state.join("operations.db"))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS schema_version(version INTEGER NOT NULL);
            INSERT INTO schema_version SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM schema_version);
            CREATE TABLE IF NOT EXISTS plans(id TEXT PRIMARY KEY, body TEXT NOT NULL, updated_ms TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS events(sequence INTEGER PRIMARY KEY, plan_id TEXT NOT NULL REFERENCES plans(id), at_ms TEXT NOT NULL, state TEXT NOT NULL, detail TEXT);")?;
        ensure!(
            db.query_row("SELECT version FROM schema_version", [], |r| r
                .get::<_, i64>(0))?
                == 1,
            "Unsupported journal version"
        );
        Ok(Self {
            db,
            scope: Scope::new(roots, state)?,
            _lock: lock,
        })
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// Preview persists only a journal entry. It never creates destination or staging files.
    pub fn prepare(&mut self, requests: Vec<Request>) -> Result<Plan> {
        ensure!(!requests.is_empty(), "Select at least one file");
        let id = uuid::Uuid::new_v4().to_string();
        let mut plan = Plan {
            id,
            created_ms: now(),
            state: PlanState::Prepared,
            rollback_requested: false,
            items: vec![],
            conflicts: vec![],
            error: None,
        };
        let mut sources = HashSet::new();
        let mut destinations = HashSet::new();
        for mut request in requests {
            request.source = self.scope.checked(&request.source, false)?;
            request.destination = self.scope.checked(&request.destination, true)?;
            // JSON journal v1 deliberately rejects non-Unicode operation paths instead of corrupting them.
            ensure!(
                request.source.to_str().is_some() && request.destination.to_str().is_some(),
                "Non-Unicode paths can be browsed but are not supported by journal v1"
            );
            let expected = filesystem::snapshot(&request.source)?;
            if request.source == request.destination {
                plan.conflicts.push(format!(
                    "Source equals destination: {}",
                    request.source.display()
                ));
            }
            if !sources.insert(request.source.clone()) {
                plan.conflicts.push(format!(
                    "Source selected twice: {}",
                    request.source.display()
                ));
            }
            if !destinations.insert(request.destination.clone()) {
                plan.conflicts.push(format!(
                    "Two files target the same destination: {}",
                    request.destination.display()
                ));
            }
            if let Err(e) = filesystem::absent(&request.destination) {
                plan.conflicts.push(e.to_string());
            }
            let index = plan.items.len();
            let stage = request
                .destination
                .parent()
                .unwrap()
                .join(format!(".filespring-{}-{index}.part", plan.id));
            let undo_stage = request
                .source
                .parent()
                .unwrap()
                .join(format!(".filespring-{}-{index}.undo", plan.id));
            plan.items.push(Item {
                request,
                expected,
                state: ItemState::Pending,
                stage,
                stage_identity: None,
                staged: None,
                undo_stage,
                undo_identity: None,
                undo_snapshot: None,
            });
        }
        for item in &plan.items {
            if sources.contains(&item.request.destination) {
                plan.conflicts.push("A destination is also a selected source; split chained renames into separate plans".into());
            }
        }
        self.save(&plan, "Preview prepared; no file changes")?;
        Ok(plan)
    }

    pub fn load(&self, id: &str) -> Result<Plan> {
        let body: String = self
            .db
            .query_row("SELECT body FROM plans WHERE id=?1", [id], |r| r.get(0))
            .context("Plan not found")?;
        Ok(serde_json::from_str(&body)?)
    }
    pub fn history(&self) -> Result<Vec<Plan>> {
        let mut stmt = self
            .db
            .prepare("SELECT body FROM plans ORDER BY rowid DESC LIMIT 200")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn events(&self, id: &str) -> Result<Vec<String>> {
        let mut stmt = self.db.prepare("SELECT at_ms || ' ' || state || ' ' || COALESCE(detail,'') FROM events WHERE plan_id=?1 ORDER BY sequence")?;
        Ok(stmt
            .query_map([id], |r| r.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }
    fn save(&mut self, plan: &Plan, detail: &str) -> Result<()> {
        let tx = self.db.transaction()?;
        tx.execute("INSERT INTO plans(id,body,updated_ms) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET body=excluded.body,updated_ms=excluded.updated_ms", params![plan.id, serde_json::to_string(plan)?, now().to_string()])?;
        tx.execute(
            "INSERT INTO events(plan_id,at_ms,state,detail) VALUES(?1,?2,?3,?4)",
            params![
                plan.id,
                now().to_string(),
                format!("{:?}", plan.state),
                detail
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
    fn check_item(&self, item: &Item) -> Result<()> {
        self.scope.checked(&item.request.source, true)?;
        self.scope.checked(&item.request.destination, true)?;
        self.scope.check_internal(&item.stage)?;
        self.scope.check_internal(&item.undo_stage)?;
        Ok(())
    }

    pub fn execute(&mut self, id: &str) -> Result<Plan> {
        self.execute_with_progress(id, |_, _| {})
    }

    /// Explicit resume is idempotent. A process restart never auto-resumes a mutation.
    pub fn execute_with_progress(
        &mut self,
        id: &str,
        mut progress: impl FnMut(usize, usize),
    ) -> Result<Plan> {
        let mut plan = self.load(id)?;
        ensure!(
            !plan.rollback_requested,
            "This plan is being rolled back; resume rollback instead"
        );
        ensure!(
            plan.executable(),
            "Plan contains conflicts and cannot execute"
        );
        ensure!(
            plan.state != PlanState::RolledBack,
            "A rolled back plan cannot execute again"
        );
        if plan.state == PlanState::Completed {
            return Ok(plan);
        }
        // Preflight the WHOLE batch before the first write. Recheck each item immediately before use.
        let preflight = (|| -> Result<()> {
            for item in &plan.items {
                self.check_item(item)?;
                if item.state == ItemState::Pending {
                    filesystem::verify(&item.request.source, &item.expected)?;
                    filesystem::absent(&item.request.destination)?;
                }
            }
            let mut needed = std::collections::HashMap::<PathBuf, u64>::new();
            for item in &plan.items {
                if matches!(item.state, ItemState::Pending | ItemState::Staging) {
                    *needed
                        .entry(item.request.destination.parent().unwrap().to_path_buf())
                        .or_default() += item.expected.bytes;
                }
            }
            for (dir, bytes) in needed {
                ensure!(
                    fs2::available_space(dir)? >= bytes,
                    "Not enough destination disk space"
                );
            }
            Ok(())
        })();
        if let Err(e) = preflight {
            return self.fail(&mut plan, e);
        }
        plan.state = PlanState::Running;
        plan.error = None;
        self.save(&plan, "Execution explicitly requested")?;
        for index in 0..plan.items.len() {
            if let Err(e) = self.run_item(&mut plan, index) {
                return self.fail(&mut plan, e);
            }
            progress(index + 1, plan.items.len());
        }
        plan.state = PlanState::Completed;
        self.save(&plan, "All destinations verified")?;
        Ok(plan)
    }

    fn fail(&mut self, plan: &mut Plan, e: anyhow::Error) -> Result<Plan> {
        plan.state = if plan.items.iter().any(|i| i.state != ItemState::Pending) {
            PlanState::PartiallyCompleted
        } else {
            PlanState::Failed
        };
        plan.error = Some(format!("{e:#}"));
        self.save(plan, "Stopped; inspect journal before resuming")?;
        Err(e)
    }

    fn run_item(&mut self, plan: &mut Plan, index: usize) -> Result<()> {
        self.check_item(&plan.items[index])?;
        if plan.items[index].state == ItemState::Completed {
            self.cleanup_stage(&plan.items[index], false)?;
            return Ok(());
        }
        if matches!(
            plan.items[index].state,
            ItemState::Pending | ItemState::Staging
        ) {
            self.make_stage(plan, index, false)?;
            plan.items[index].state = ItemState::Staged;
            self.save(plan, "Staging data verified and durable")?;
        }
        if plan.items[index].state == ItemState::Staged {
            let item = &plan.items[index];
            let staged = item.staged.as_ref().context("Missing staged fingerprint")?;
            filesystem::verify(&item.stage, staged)?;
            self.publish(&item.stage, &item.request.destination, staged)?;
            plan.items[index].state = ItemState::Published;
            self.save(plan, "Destination published without overwrite")?;
        }
        if plan.items[index].state == ItemState::Published {
            let item = &plan.items[index];
            filesystem::verify(&item.request.destination, item.staged.as_ref().unwrap())?;
            if item.request.action == Action::Move {
                match fs::symlink_metadata(&item.request.source) {
                    Ok(_) => filesystem::remove_owned(&item.request.source, &item.expected)?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            plan.items[index].state = ItemState::Completed;
            self.save(plan, "File complete; source disposition recorded")?;
            self.cleanup_stage(&plan.items[index], false)?;
        }
        ensure!(
            plan.items[index].state == ItemState::Completed,
            "Unexpected item state"
        );
        Ok(())
    }

    fn make_stage(&mut self, plan: &mut Plan, index: usize, undo: bool) -> Result<()> {
        let item = &plan.items[index];
        let (source, expected, stage, prior_id) = if undo {
            (
                &item.request.destination,
                item.staged
                    .as_ref()
                    .context("Missing destination fingerprint")?,
                &item.undo_stage,
                &item.undo_identity,
            )
        } else {
            (
                &item.request.source,
                &item.expected,
                &item.stage,
                &item.stage_identity,
            )
        };
        filesystem::verify(source, expected)?;
        if filesystem::exists(stage)? {
            ensure!(
                prior_id
                    .as_ref()
                    .is_some_and(|id| filesystem::identity(stage).ok().as_ref() == Some(id)),
                "Unknown staging file; preserve for manual inspection: {}",
                stage.display()
            );
            // A previously interrupted, unpublished write may be discarded only with ownership proof.
            fs::remove_file(stage)?;
            filesystem::sync_parent(stage)?;
        }
        let source = source.clone();
        let expected = expected.clone();
        let stage = stage.clone();
        let mut file = filesystem::new_stage(&stage)?;
        filesystem::sync_parent(&stage)?;
        let identity = filesystem::identity(&stage)?;
        if undo {
            plan.items[index].undo_identity = Some(identity);
        } else {
            plan.items[index].stage_identity = Some(identity);
            plan.items[index].state = ItemState::Staging;
        }
        self.save(plan, "Staging identity recorded before writing")?;
        filesystem::write_stage(&source, &expected, &mut file)?;
        let staged = filesystem::snapshot(&stage)?;
        ensure!(
            staged.bytes == expected.bytes && staged.hash == expected.hash,
            "Staged verification failed"
        );
        if undo {
            plan.items[index].undo_snapshot = Some(staged);
        } else {
            plan.items[index].staged = Some(staged);
        }
        Ok(())
    }

    /// Hard-link publication is atomic and never replaces an existing path. Unsupported
    /// filesystems fail safely; there is no unsafe rename-overwrite fallback.
    fn publish(&self, stage: &Path, destination: &Path, staged: &Snapshot) -> Result<()> {
        match fs::symlink_metadata(destination) {
            Ok(_) => filesystem::verify(destination, staged)
                .context("Destination occupied by another file")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::hard_link(stage, destination).context(
                    "Cannot publish safely; destination filesystem must support hard links",
                )?;
                filesystem::sync_parent(destination)?;
            }
            Err(e) => return Err(e.into()),
        }
        filesystem::verify(destination, staged)
    }

    fn cleanup_stage(&self, item: &Item, undo: bool) -> Result<()> {
        let (path, expected) = if undo {
            (&item.undo_stage, item.undo_snapshot.as_ref())
        } else {
            (&item.stage, item.staged.as_ref())
        };
        if filesystem::exists(path)? {
            self.scope.check_internal(path)?;
            filesystem::remove_owned(
                path,
                expected.context("Unknown recovery artifact; preserve for manual inspection")?,
            )?;
        }
        Ok(())
    }

    pub fn rollback(&mut self, id: &str) -> Result<Plan> {
        let mut plan = self.load(id)?;
        if plan.state == PlanState::RolledBack {
            return Ok(plan);
        }
        // A staging/published item first finishes reconciliation to keep a single direction of recovery.
        if !plan.rollback_requested {
            for index in 0..plan.items.len() {
                if matches!(
                    plan.items[index].state,
                    ItemState::Staging | ItemState::Staged | ItemState::Published
                ) && let Err(e) = self.run_item(&mut plan, index)
                {
                    return self.fail(&mut plan, e);
                }
            }
            for item in &plan.items {
                self.check_item(item)?;
                if item.state == ItemState::Completed {
                    filesystem::verify(
                        &item.request.destination,
                        item.staged.as_ref().context("Missing fingerprint")?,
                    )?;
                    if item.request.action == Action::Move {
                        filesystem::absent(&item.request.source)?;
                    }
                }
            }
            plan.rollback_requested = true;
        }
        plan.state = PlanState::RollingBack;
        plan.error = None;
        self.save(&plan, "Rollback explicitly requested")?;
        for index in (0..plan.items.len()).rev() {
            if let Err(e) = self.undo_item(&mut plan, index) {
                return self.fail(&mut plan, e);
            }
        }
        plan.state = PlanState::RolledBack;
        self.save(&plan, "Rollback complete; not a backup")?;
        Ok(plan)
    }

    fn undo_item(&mut self, plan: &mut Plan, index: usize) -> Result<()> {
        self.check_item(&plan.items[index])?;
        if plan.items[index].state == ItemState::Pending {
            plan.items[index].state = ItemState::RolledBack;
            self.save(plan, "Unstarted item cancelled")?;
        }
        if plan.items[index].state == ItemState::RolledBack {
            self.cleanup_stage(&plan.items[index], true)?;
            return Ok(());
        }
        if plan.items[index].state == ItemState::Completed {
            plan.items[index].state = ItemState::Reverting;
            self.save(plan, "Undo intent recorded")?;
        }
        if plan.items[index].state == ItemState::Reverting {
            if plan.items[index].request.action == Action::Move {
                // After a crash, an already-published restored source is identified by its saved fingerprint.
                let item = &plan.items[index];
                let published = item
                    .undo_snapshot
                    .as_ref()
                    .is_some_and(|s| filesystem::verify(&item.request.source, s).is_ok());
                if !published {
                    filesystem::absent(&item.request.source)?;
                    let stage_ready = item
                        .undo_snapshot
                        .as_ref()
                        .is_some_and(|s| filesystem::verify(&item.undo_stage, s).is_ok());
                    if !stage_ready {
                        self.make_stage(plan, index, true)?;
                        self.save(plan, "Undo staging verified")?;
                    }
                    let item = &plan.items[index];
                    self.publish(
                        &item.undo_stage,
                        &item.request.source,
                        item.undo_snapshot.as_ref().unwrap(),
                    )?;
                }
            }
            plan.items[index].state = ItemState::Restored;
            self.save(plan, "Restored source is durable, or copy ready to remove")?;
        }
        if plan.items[index].state == ItemState::Restored {
            let item = &plan.items[index];
            if item.request.action == Action::Move {
                filesystem::verify(
                    &item.request.source,
                    item.undo_snapshot
                        .as_ref()
                        .context("Missing restored fingerprint")?,
                )?;
            }
            if filesystem::exists(&item.request.destination)? {
                filesystem::remove_owned(
                    &item.request.destination,
                    item.staged
                        .as_ref()
                        .context("Missing destination fingerprint")?,
                )?;
            }
            plan.items[index].state = ItemState::RolledBack;
            self.save(plan, "Undo item complete")?;
            self.cleanup_stage(&plan.items[index], true)?;
            self.cleanup_stage(&plan.items[index], false)?;
        }
        ensure!(
            plan.items[index].state == ItemState::RolledBack,
            "Unexpected undo state"
        );
        Ok(())
    }
}

fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    struct Fixture {
        _temp: tempfile::TempDir,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(temp.path()).unwrap();
            fs::create_dir(root.join("out")).unwrap();
            fs::write(root.join("source"), b"recoverable").unwrap();
            Self { _temp: temp, root }
        }
        fn open(&self) -> Engine {
            Engine::open(&self.root.join("state"), std::slice::from_ref(&self.root)).unwrap()
        }
        fn prepare(&self, e: &mut Engine) -> Plan {
            e.prepare(vec![Request {
                action: Action::Move,
                source: self.root.join("source"),
                destination: self.root.join("out/dest"),
            }])
            .unwrap()
        }
    }
    #[test]
    fn recover_partial_stage_without_touching_source() {
        let f = Fixture::new();
        let id = {
            let mut e = f.open();
            let mut p = f.prepare(&mut e);
            fs::write(&p.items[0].stage, b"incomplete").unwrap();
            p.items[0].stage_identity = Some(filesystem::identity(&p.items[0].stage).unwrap());
            p.items[0].state = ItemState::Staging;
            p.state = PlanState::Running;
            e.save(&p, "simulated interruption after partial write")
                .unwrap();
            p.id
        };
        let mut e = f.open();
        e.execute(&id).unwrap();
        assert_eq!(fs::read(f.root.join("out/dest")).unwrap(), b"recoverable");
    }
    #[test]
    fn preserve_unknown_staging_artifact() {
        let f = Fixture::new();
        let mut e = f.open();
        let p = f.prepare(&mut e);
        fs::write(&p.items[0].stage, b"unknown owner").unwrap();
        assert!(e.execute(&p.id).is_err());
        assert!(f.root.join("source").exists());
        assert_eq!(fs::read(&p.items[0].stage).unwrap(), b"unknown owner");
    }

    #[test]
    fn staged_destination_conflict_never_clobbers_and_can_resume_after_resolution() {
        let f = Fixture::new();
        let mut e = f.open();
        let mut p = f.prepare(&mut e);
        e.make_stage(&mut p, 0, false).unwrap();
        p.items[0].state = ItemState::Staged;
        e.save(&p, "staged before conflict").unwrap();
        fs::write(f.root.join("out/dest"), b"external").unwrap();
        assert!(e.execute(&p.id).is_err());
        assert_eq!(fs::read(f.root.join("out/dest")).unwrap(), b"external");
        assert!(f.root.join("source").exists());
        fs::remove_file(f.root.join("out/dest")).unwrap();
        assert_eq!(e.execute(&p.id).unwrap().state, PlanState::Completed);
    }

    #[test]
    fn published_content_change_preserves_original_source() {
        let f = Fixture::new();
        let mut e = f.open();
        let mut p = f.prepare(&mut e);
        e.make_stage(&mut p, 0, false).unwrap();
        e.publish(
            &p.items[0].stage,
            &p.items[0].request.destination,
            p.items[0].staged.as_ref().unwrap(),
        )
        .unwrap();
        p.items[0].state = ItemState::Published;
        e.save(&p, "published").unwrap();
        fs::write(f.root.join("out/dest"), b"external edit").unwrap();
        assert!(e.execute(&p.id).is_err());
        assert_eq!(fs::read(f.root.join("source")).unwrap(), b"recoverable");
    }
    #[test]
    fn recover_publish_before_journal_update() {
        let f = Fixture::new();
        let id = {
            let mut e = f.open();
            let mut p = f.prepare(&mut e);
            e.make_stage(&mut p, 0, false).unwrap();
            p.items[0].state = ItemState::Staged;
            p.state = PlanState::Running;
            e.save(&p, "staged").unwrap();
            fs::hard_link(&p.items[0].stage, &p.items[0].request.destination).unwrap();
            p.id
        };
        let mut e = f.open();
        e.execute(&id).unwrap();
        assert!(!f.root.join("source").exists());
        e.rollback(&id).unwrap();
        assert_eq!(fs::read(f.root.join("source")).unwrap(), b"recoverable");
    }
    #[test]
    fn recover_source_unlink_before_completed_journal() {
        let f = Fixture::new();
        let id = {
            let mut e = f.open();
            let mut p = f.prepare(&mut e);
            e.make_stage(&mut p, 0, false).unwrap();
            e.publish(
                &p.items[0].stage,
                &p.items[0].request.destination,
                p.items[0].staged.as_ref().unwrap(),
            )
            .unwrap();
            p.items[0].state = ItemState::Published;
            p.state = PlanState::Running;
            e.save(&p, "published").unwrap();
            fs::remove_file(f.root.join("source")).unwrap();
            p.id
        };
        let mut e = f.open();
        assert_eq!(e.execute(&id).unwrap().state, PlanState::Completed);
        e.rollback(&id).unwrap();
        assert!(f.root.join("source").exists());
    }
    #[test]
    fn recover_undo_publication_before_journal_update() {
        let f = Fixture::new();
        let id = {
            let mut e = f.open();
            let p = f.prepare(&mut e);
            let mut p = e.execute(&p.id).unwrap();
            p.rollback_requested = true;
            p.state = PlanState::RollingBack;
            p.items[0].state = ItemState::Reverting;
            e.make_stage(&mut p, 0, true).unwrap();
            e.save(&p, "undo staged").unwrap();
            e.publish(
                &p.items[0].undo_stage,
                &p.items[0].request.source,
                p.items[0].undo_snapshot.as_ref().unwrap(),
            )
            .unwrap();
            p.id
        };
        let mut e = f.open();
        assert_eq!(e.rollback(&id).unwrap().state, PlanState::RolledBack);
        assert!(f.root.join("source").exists());
        assert!(!f.root.join("out/dest").exists());
    }
    #[test]
    fn recover_undo_removal_before_journal_update() {
        let f = Fixture::new();
        let id = {
            let mut e = f.open();
            let p = f.prepare(&mut e);
            let mut p = e.execute(&p.id).unwrap();
            p.rollback_requested = true;
            p.state = PlanState::RollingBack;
            p.items[0].state = ItemState::Reverting;
            e.make_stage(&mut p, 0, true).unwrap();
            e.publish(
                &p.items[0].undo_stage,
                &p.items[0].request.source,
                p.items[0].undo_snapshot.as_ref().unwrap(),
            )
            .unwrap();
            p.items[0].state = ItemState::Restored;
            e.save(&p, "undo source restored").unwrap();
            fs::remove_file(&p.items[0].request.destination).unwrap();
            p.id
        };
        let mut e = f.open();
        assert_eq!(e.rollback(&id).unwrap().state, PlanState::RolledBack);
        assert_eq!(fs::read(f.root.join("source")).unwrap(), b"recoverable");
    }
}
