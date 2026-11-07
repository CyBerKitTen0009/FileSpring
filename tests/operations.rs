use filespring::{Action, Engine, PlanState, Request, browser};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
};

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir(root.join("in")).unwrap();
        fs::create_dir(root.join("out")).unwrap();
        let state = root.join("state");
        Self {
            _temp: temp,
            root,
            state,
        }
    }
    fn engine(&self) -> Engine {
        Engine::open(&self.state, std::slice::from_ref(&self.root)).unwrap()
    }
    fn request(&self, action: Action, name: &str) -> Request {
        Request {
            action,
            source: self.root.join("in").join(name),
            destination: self.root.join("out").join(name),
        }
    }
    fn file(&self, name: &str, data: &[u8]) {
        fs::write(self.root.join("in").join(name), data).unwrap();
    }
}

#[test]
fn preview_does_not_modify_files() {
    let f = Fixture::new();
    f.file("报告 中文.txt", b"hello");
    let p = f
        .engine()
        .prepare(vec![f.request(Action::Move, "报告 中文.txt")])
        .unwrap();
    assert_eq!(p.state, PlanState::Prepared);
    assert!(p.executable());
    assert_eq!(p.bytes(), 5);
    assert!(f.root.join("in/报告 中文.txt").exists());
    assert_eq!(fs::read_dir(f.root.join("out")).unwrap().count(), 0);
}

#[test]
fn copy_verify_reopen_and_undo() {
    let f = Fixture::new();
    f.file("a", b"contents");
    let id = {
        let mut e = f.engine();
        let p = e.prepare(vec![f.request(Action::Copy, "a")]).unwrap();
        assert_eq!(e.execute(&p.id).unwrap().state, PlanState::Completed);
        p.id
    };
    assert_eq!(fs::read(f.root.join("out/a")).unwrap(), b"contents");
    let mut e = f.engine();
    assert_eq!(e.history().unwrap()[0].id, id);
    assert_eq!(e.rollback(&id).unwrap().state, PlanState::RolledBack);
    assert!(f.root.join("in/a").exists());
    assert!(!f.root.join("out/a").exists());
    assert_eq!(e.rollback(&id).unwrap().state, PlanState::RolledBack);
    assert!(e.events(&id).unwrap().len() >= 8);
}

#[test]
fn move_and_undo_preserve_bytes_and_mtime() {
    let f = Fixture::new();
    f.file("a", &[0, 1, 2, 255]);
    let before = fs::metadata(f.root.join("in/a"))
        .unwrap()
        .modified()
        .unwrap();
    let mut e = f.engine();
    let p = e.prepare(vec![f.request(Action::Move, "a")]).unwrap();
    e.execute(&p.id).unwrap();
    assert!(!f.root.join("in/a").exists());
    assert_eq!(
        fs::metadata(f.root.join("out/a"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
    e.rollback(&p.id).unwrap();
    assert_eq!(fs::read(f.root.join("in/a")).unwrap(), [0, 1, 2, 255]);
    assert_eq!(
        fs::metadata(f.root.join("in/a"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
    assert!(!f.root.join("out/a").exists());
}

#[test]
fn existing_destination_is_a_blocking_conflict() {
    let f = Fixture::new();
    f.file("a", b"source");
    fs::write(f.root.join("out/a"), b"keep").unwrap();
    let mut e = f.engine();
    let p = e.prepare(vec![f.request(Action::Copy, "a")]).unwrap();
    assert!(!p.executable());
    assert!(e.execute(&p.id).is_err());
    assert_eq!(fs::read(f.root.join("out/a")).unwrap(), b"keep");
}

#[test]
fn source_changed_after_preview_stops_entire_batch() {
    let f = Fixture::new();
    f.file("a", b"old");
    f.file("b", b"old");
    let mut e = f.engine();
    let p = e
        .prepare(vec![
            f.request(Action::Move, "a"),
            f.request(Action::Move, "b"),
        ])
        .unwrap();
    fs::write(f.root.join("in/b"), b"new").unwrap();
    assert!(e.execute(&p.id).is_err());
    assert!(f.root.join("in/a").exists());
    assert_eq!(fs::read_dir(f.root.join("out")).unwrap().count(), 0);
    assert_eq!(e.load(&p.id).unwrap().state, PlanState::Failed);
}

#[test]
fn destination_created_after_preview_is_never_overwritten() {
    let f = Fixture::new();
    f.file("a", b"source");
    let mut e = f.engine();
    let p = e.prepare(vec![f.request(Action::Move, "a")]).unwrap();
    fs::write(f.root.join("out/a"), b"unrelated").unwrap();
    assert!(e.execute(&p.id).is_err());
    assert!(f.root.join("in/a").exists());
    assert_eq!(fs::read(f.root.join("out/a")).unwrap(), b"unrelated");
}

#[test]
fn undo_refuses_modified_destination() {
    let f = Fixture::new();
    f.file("a", b"original");
    let mut e = f.engine();
    let p = e.prepare(vec![f.request(Action::Copy, "a")]).unwrap();
    e.execute(&p.id).unwrap();
    fs::write(f.root.join("out/a"), b"edited by user").unwrap();
    assert!(e.rollback(&p.id).is_err());
    assert_eq!(fs::read(f.root.join("out/a")).unwrap(), b"edited by user");
}

#[test]
fn undo_refuses_same_content_different_identity() {
    let f = Fixture::new();
    f.file("a", b"same");
    let mut e = f.engine();
    let p = e.prepare(vec![f.request(Action::Copy, "a")]).unwrap();
    e.execute(&p.id).unwrap();
    fs::write(f.root.join("out/replacement"), b"same").unwrap();
    fs::rename(f.root.join("out/replacement"), f.root.join("out/a")).unwrap();
    assert!(e.rollback(&p.id).is_err());
    assert!(f.root.join("out/a").exists());
}

#[test]
fn undo_move_refuses_recreated_original_name() {
    let f = Fixture::new();
    f.file("a", b"old");
    let mut e = f.engine();
    let p = e.prepare(vec![f.request(Action::Move, "a")]).unwrap();
    e.execute(&p.id).unwrap();
    f.file("a", b"new");
    assert!(e.rollback(&p.id).is_err());
    assert_eq!(fs::read(f.root.join("in/a")).unwrap(), b"new");
    assert!(f.root.join("out/a").exists());
}

#[test]
fn duplicate_destinations_and_chained_renames_are_blocked() {
    let f = Fixture::new();
    f.file("a", b"a");
    f.file("b", b"b");
    let mut e = f.engine();
    let mut b = f.request(Action::Copy, "b");
    b.destination = f.root.join("out/a");
    assert!(
        !e.prepare(vec![f.request(Action::Copy, "a"), b])
            .unwrap()
            .executable()
    );
    let mut a = f.request(Action::Move, "a");
    a.destination = f.root.join("in/b");
    assert!(
        !e.prepare(vec![a, f.request(Action::Move, "b")])
            .unwrap()
            .executable()
    );
}

#[test]
fn scope_prevents_escape_and_journal_access() {
    let f = Fixture::new();
    f.file("a", b"a");
    let mut e = f.engine();
    let mut r = f.request(Action::Copy, "a");
    r.destination = f.root.join("in/../../escape");
    assert!(e.prepare(vec![r]).is_err());
    let mut r = f.request(Action::Copy, "a");
    r.destination = f.state.join("evil");
    assert!(e.prepare(vec![r]).is_err());
    let other = tempfile::tempdir().unwrap();
    let mut r = f.request(Action::Copy, "a");
    r.destination = fs::canonicalize(other.path()).unwrap().join("escape");
    assert!(e.prepare(vec![r]).is_err());
}

#[test]
fn directories_are_not_misrepresented_as_supported_operations() {
    let f = Fixture::new();
    let mut e = f.engine();
    assert!(
        e.prepare(vec![Request {
            action: Action::Move,
            source: f.root.join("in"),
            destination: f.root.join("out/in")
        }])
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn unreadable_source_is_rejected_during_preview() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.file("a", b"private");
    let mut e = f.engine();
    let path = f.root.join("in/a");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let result = e.prepare(vec![f.request(Action::Copy, "a")]);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(result.is_err());
    assert!(!f.root.join("out/a").exists());
}

#[cfg(unix)]
#[test]
fn inaccessible_paths_are_errors_instead_of_missing() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    fs::write(f.root.join("out/keep"), b"keep").unwrap();
    fs::set_permissions(f.root.join("out"), fs::Permissions::from_mode(0o000)).unwrap();
    let result = filespring::filesystem::exists(&f.root.join("out/keep"));
    fs::set_permissions(f.root.join("out"), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(filespring::filesystem::exists(&f.root.join("out/keep")).unwrap());
    assert!(!filespring::filesystem::exists(&f.root.join("out/missing")).unwrap());
}

#[cfg(unix)]
#[test]
fn replacing_parent_with_link_after_preview_is_rejected() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.file("a", b"safe");
    let mut e = f.engine();
    let p = e.prepare(vec![f.request(Action::Move, "a")]).unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::rename(f.root.join("out"), f.root.join("old-out")).unwrap();
    symlink(outside.path(), f.root.join("out")).unwrap();
    assert!(e.execute(&p.id).is_err());
    assert!(f.root.join("in/a").exists());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn exclusive_windows_lock_is_reported_without_modifications() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    f.file("a", b"locked");
    let mut e = f.engine();
    let _locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(f.root.join("in/a"))
        .unwrap();
    assert!(e.prepare(vec![f.request(Action::Move, "a")]).is_err());
    assert!(!f.root.join("out/a").exists());
}

#[cfg(windows)]
#[test]
fn windows_lock_after_preview_stops_before_destination_changes() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    f.file("a", b"locked after preview");
    let mut e = f.engine();
    let plan = e.prepare(vec![f.request(Action::Move, "a")]).unwrap();
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(f.root.join("in/a"))
        .unwrap();
    assert!(e.execute(&plan.id).is_err());
    assert!(!f.root.join("out/a").exists());
    drop(locked);
    assert_eq!(
        fs::read(f.root.join("in/a")).unwrap(),
        b"locked after preview"
    );
}

#[cfg(windows)]
#[test]
fn windows_readonly_source_is_rejected_before_staging() {
    let f = Fixture::new();
    f.file("a", b"read only");
    let path = f.root.join("in/a");
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let result = f.engine().prepare(vec![f.request(Action::Copy, "a")]);
    fs::set_permissions(&path, original).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read_dir(f.root.join("out")).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn windows_device_aliases_and_ads_never_reach_staging() {
    let f = Fixture::new();
    f.file("a", b"safe");
    let mut e = f.engine();
    for name in [
        "NUL",
        "nul.txt",
        "a:stream",
        "CON",
        "trailing.",
        "trailing ",
    ] {
        let mut request = f.request(Action::Copy, "a");
        request.destination = f.root.join("out").join(name);
        assert!(e.prepare(vec![request]).is_err(), "{name}");
    }
    assert_eq!(fs::read_dir(f.root.join("out")).unwrap().count(), 0);
    assert_eq!(fs::read(f.root.join("in/a")).unwrap(), b"safe");
}

#[cfg(windows)]
#[test]
fn windows_verbatim_long_path_copy_and_undo() {
    let f = Fixture::new();
    let mut source = f.root.join("in");
    let mut destination = f.root.join("out");
    for _ in 0..5 {
        source.push("long-directory-component-012345678901234567890123456789");
        destination.push("long-directory-component-012345678901234567890123456789");
    }
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&destination).unwrap();
    source.push("中文.txt");
    destination.push("中文.txt");
    assert!(source.as_os_str().len() > 260);
    fs::write(&source, b"long path").unwrap();
    let mut e = f.engine();
    let plan = e
        .prepare(vec![Request {
            action: Action::Copy,
            source: source.clone(),
            destination: destination.clone(),
        }])
        .unwrap();
    assert_eq!(e.execute(&plan.id).unwrap().state, PlanState::Completed);
    assert_eq!(fs::read(&destination).unwrap(), b"long path");
    e.rollback(&plan.id).unwrap();
    assert!(source.exists());
    assert!(!destination.exists());
}

#[cfg(windows)]
#[test]
fn windows_junction_cannot_redirect_a_prepared_move() {
    let f = Fixture::new();
    f.file("a", b"safe");
    let mut e = f.engine();
    let plan = e.prepare(vec![f.request(Action::Move, "a")]).unwrap();
    fs::rename(f.root.join("out"), f.root.join("old-out")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let junction = f.root.join("out");
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:FILESPRING_TEST_LINK -Target $env:FILESPRING_TEST_TARGET -ErrorAction Stop | Out-Null"])
        .env("FILESPRING_TEST_LINK", junction.to_string_lossy().trim_start_matches(r"\\?\"))
        .env("FILESPRING_TEST_TARGET", outside.path())
        .status().unwrap();
    assert!(status.success());
    let result = e.execute(&plan.id);
    // Remove the junction itself before fixture cleanup; never recurse into its target.
    fs::remove_dir(&junction).unwrap();
    assert!(result.is_err());
    assert!(f.root.join("in/a").exists());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn portable_self_test_uses_a_new_fixture_and_refuses_existing_data() {
    let parent = tempfile::tempdir().unwrap();
    let fixture = parent.path().join("generated fixture");
    let unused_state = parent.path().join("must-not-create-state");
    let output = Command::new(env!("CARGO_BIN_EXE_filespring"))
        .arg("--state")
        .arg(&unused_state)
        .arg("self-test")
        .arg("--fixture")
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["result"], "passed");
    assert_eq!(report["checks"].as_array().unwrap().len(), 9);
    assert!(!unused_state.exists());
    let before = fs::read(fixture.join("verification.json")).unwrap();
    fs::write(fixture.join("keep"), b"must remain").unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_filespring"))
        .arg("self-test")
        .arg("--fixture")
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert_eq!(fs::read(fixture.join("keep")).unwrap(), b"must remain");
    assert_eq!(fs::read(fixture.join("verification.json")).unwrap(), before);
}

#[test]
fn zero_byte_long_unicode_and_special_names() {
    let f = Fixture::new();
    let name = format!("中文 空 & $ ` ' {}.txt", "x".repeat(120));
    f.file(&name, b"");
    let mut e = f.engine();
    let p = e.prepare(vec![f.request(Action::Copy, &name)]).unwrap();
    e.execute(&p.id).unwrap();
    assert_eq!(
        fs::metadata(f.root.join("out").join(&name)).unwrap().len(),
        0
    );
    e.rollback(&p.id).unwrap();
}

#[test]
fn journal_is_single_writer() {
    let f = Fixture::new();
    let _e = f.engine();
    assert!(Engine::open(&f.state, std::slice::from_ref(&f.root)).is_err());
}

#[test]
fn scan_streams_batches_and_can_cancel() {
    let f = Fixture::new();
    for i in 0..1000 {
        f.file(&format!("{i}"), b"");
    }
    let e = f.engine();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut batches = 0;
    let mut seen = 0;
    assert_eq!(
        browser::scan(e.scope(), &f.root.join("in"), &cancel, |batch| {
            batches += 1;
            seen += batch.len();
        })
        .unwrap(),
        1000
    );
    assert_eq!(seen, 1000);
    assert_eq!(batches, 4);
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        browser::scan(e.scope(), &f.root.join("in"), &cancel, |_| panic!(
            "cancelled"
        ))
        .unwrap(),
        0
    );
}

#[cfg(unix)]
#[test]
fn symlink_and_broken_link_are_visible_but_never_followed() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.file("a", b"a");
    symlink(f.root.join("in/a"), f.root.join("in/link")).unwrap();
    symlink("missing", f.root.join("in/broken")).unwrap();
    symlink(f.root.join("out"), f.root.join("linked-directory")).unwrap();
    let mut e = f.engine();
    assert!(e.prepare(vec![f.request(Action::Copy, "link")]).is_err());
    let mut r = f.request(Action::Copy, "a");
    r.destination = f.root.join("linked-directory/a");
    assert!(e.prepare(vec![r]).is_err());
    let mut links = 0;
    browser::scan(
        e.scope(),
        &f.root.join("in"),
        &Arc::new(AtomicBool::new(false)),
        |b| {
            links += b.iter().filter(|x| x.link).count();
        },
    )
    .unwrap();
    assert_eq!(links, 2);
}

#[cfg(target_os = "linux")]
#[test]
fn non_unicode_names_are_browsable_and_mutation_is_rejected() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    let name = std::ffi::OsString::from_vec(vec![0x66, 0xff]);
    fs::write(f.root.join("in").join(&name), b"a").unwrap();
    let mut e = f.engine();
    assert!(
        e.prepare(vec![Request {
            action: Action::Copy,
            source: f.root.join("in").join(&name),
            destination: f.root.join("out").join(&name)
        }])
        .is_err()
    );
    let mut count = 0;
    browser::scan(
        e.scope(),
        &f.root.join("in"),
        &Arc::new(AtomicBool::new(false)),
        |b| count += b.len(),
    )
    .unwrap();
    assert_eq!(count, 1);
}

// Child exits without destructors after the first committed file, so OS locks and
// SQLite recovery are tested across a real process boundary, not only Engine reopen.
#[test]
fn crash_child() {
    if let Ok(root) = std::env::var("FILESPRING_CRASH_FIXTURE") {
        let root = PathBuf::from(root);
        let mut e = Engine::open(&root.join("state"), std::slice::from_ref(&root)).unwrap();
        let id = fs::read_to_string(root.join("plan-id")).unwrap();
        e.execute_with_progress(&id, |_, _| std::process::exit(73))
            .unwrap();
        panic!("Child should have exited");
    }
}

#[test]
fn process_crash_preserves_partial_history_and_can_resume() {
    let f = Fixture::new();
    f.file("a", b"alpha");
    f.file("b", b"beta");
    let id = {
        let mut e = f.engine();
        e.prepare(vec![
            f.request(Action::Move, "a"),
            f.request(Action::Move, "b"),
        ])
        .unwrap()
        .id
    };
    fs::write(f.root.join("plan-id"), &id).unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_child", "--nocapture"])
        .env("FILESPRING_CRASH_FIXTURE", &f.root)
        .status()
        .unwrap();
    assert_eq!(result.code(), Some(73));
    let mut e = f.engine();
    let partial = e.load(&id).unwrap();
    assert_eq!(partial.state, PlanState::Running);
    assert_eq!(partial.completed(), 1);
    assert!(f.root.join("in/b").exists());
    assert_eq!(e.execute(&id).unwrap().state, PlanState::Completed);
    assert_eq!(e.execute(&id).unwrap().completed(), 2);
    e.rollback(&id).unwrap();
    assert_eq!(fs::read(f.root.join("in/a")).unwrap(), b"alpha");
    assert_eq!(fs::read(f.root.join("in/b")).unwrap(), b"beta");
}
