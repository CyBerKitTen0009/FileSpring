mod icons;
mod theme;
mod view;

use eframe::egui;
use filespring::{
    Action, Engine, Plan, PlanState, Request,
    browser::{self, Entry},
    filesystem::Scope,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

enum Job {
    Prepare(Vec<Request>),
    Execute(String),
    Undo(String),
    History,
    Preview(PathBuf),
}
enum Event {
    Plan(Plan),
    History(Vec<Plan>),
    Progress(usize, usize),
    Error(String),
    Idle,
    Preview(PathBuf, String),
}
enum Scan {
    Batch(Vec<Entry>),
    Done,
    Error(String),
}

#[derive(Clone)]
struct FileClipboard {
    action: Action,
    sources: Vec<PathBuf>,
    text: String,
}

impl FileClipboard {
    fn new(action: Action, sources: Vec<PathBuf>) -> Option<Self> {
        if sources.is_empty() {
            return None;
        }
        let text = sources
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        Some(Self {
            action,
            sources,
            text,
        })
    }
    fn matches(&self, text: &str) -> bool {
        self.text == text
    }
}

#[derive(Debug, PartialEq)]
enum ClipboardCommand {
    Copy,
    Cut,
    Paste(String),
}

fn clipboard_commands(ctx: &egui::Context) -> Vec<ClipboardCommand> {
    ctx.input(|input| {
        input
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Copy => Some(ClipboardCommand::Copy),
                egui::Event::Cut => Some(ClipboardCommand::Cut),
                egui::Event::Paste(text) => Some(ClipboardCommand::Paste(text.clone())),
                _ => None,
            })
            .collect()
    })
}

struct Pane {
    path: PathBuf,
    input: String,
    entries: Vec<Entry>,
    visible: Vec<usize>,
    filter: String,
    selected: BTreeSet<PathBuf>,
    anchor: usize,
    rx: Option<Receiver<Scan>>,
    cancel: Arc<AtomicBool>,
    scanning: bool,
    error: Option<String>,
    back: Vec<PathBuf>,
}
impl Pane {
    fn new(path: PathBuf) -> Self {
        Self {
            input: path.display().to_string(),
            path,
            entries: vec![],
            visible: vec![],
            filter: String::new(),
            selected: BTreeSet::new(),
            anchor: 0,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            scanning: false,
            error: None,
            back: vec![],
        }
    }
    fn navigate(&mut self, scope: &Scope, path: PathBuf, ctx: &egui::Context) {
        match scope.checked(&path, false) {
            Ok(path) if path.is_dir() => {
                if path != self.path {
                    self.back.push(self.path.clone());
                }
                self.path = path;
                self.input = self.path.display().to_string();
                self.refresh(scope, ctx);
            }
            Ok(_) => self.error = Some("Please select a directory / 请选择目录".into()),
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }
    fn refresh(&mut self, scope: &Scope, ctx: &egui::Context) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let scope = scope.clone();
        let path = self.path.clone();
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::sync_channel(8);
        self.rx = Some(rx);
        self.entries.clear();
        self.visible.clear();
        self.selected.clear();
        self.error = None;
        self.scanning = true;
        std::thread::spawn(move || {
            let result = browser::scan(&scope, &path, &cancel, |batch| {
                let _ = tx.send(Scan::Batch(batch));
                ctx.request_repaint();
            });
            let _ = tx.send(match result {
                Ok(_) => Scan::Done,
                Err(e) => Scan::Error(format!("{e:#}")),
            });
            ctx.request_repaint();
        });
    }
    fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            for _ in 0..8 {
                match rx.try_recv() {
                    Ok(Scan::Batch(entries)) => {
                        let filter = self.filter.to_lowercase();
                        for entry in entries {
                            if entry.name.to_lowercase().contains(&filter) {
                                self.visible.push(self.entries.len());
                            }
                            self.entries.push(entry);
                        }
                    }
                    Ok(Scan::Done) => self.scanning = false,
                    Ok(Scan::Error(e)) => {
                        self.scanning = false;
                        self.error = Some(e);
                    }
                    Err(_) => break,
                }
            }
        }
    }
    fn filter(&mut self) {
        let filter = self.filter.to_lowercase();
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name.to_lowercase().contains(&filter))
            .map(|(i, _)| i)
            .collect();
    }
    fn select(&mut self, row: usize, command: bool, shift: bool) {
        if !command && !shift {
            self.selected.clear();
        }
        if shift {
            for i in self.anchor.min(row)
                ..=self
                    .anchor
                    .max(row)
                    .min(self.visible.len().saturating_sub(1))
            {
                self.selected
                    .insert(self.entries[self.visible[i]].path.clone());
            }
        } else {
            let path = self.entries[self.visible[row]].path.clone();
            if command && self.selected.contains(&path) {
                self.selected.remove(&path);
            } else {
                self.selected.insert(path);
            }
            self.anchor = row;
        }
    }
}

pub struct App {
    scope: Scope,
    panes: [Pane; 2],
    active: usize,
    tx: Sender<Job>,
    rx: Receiver<Event>,
    busy: bool,
    progress: (usize, usize),
    plan: Option<Plan>,
    history: Vec<Plan>,
    activity: bool,
    clipboard: Option<FileClipboard>,
    message: String,
    zh: bool,
    light: bool,
    confirm_undo: Option<String>,
    preview: Option<(PathBuf, String)>,
    started: Instant,
    smoke_seconds: Option<u64>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        mut engine: Engine,
        smoke_seconds: Option<u64>,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        theme::install(ctx);
        let scope = engine.scope().clone();
        let root = scope.roots()[0].clone();
        let second = scope
            .roots()
            .get(1)
            .cloned()
            .unwrap_or_else(|| root.clone());
        let mut panes = [Pane::new(root), Pane::new(second)];
        for pane in &mut panes {
            pane.refresh(&scope, ctx);
        }
        let (tx, jobs) = mpsc::channel();
        let (events, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                let result = match job {
                    Job::Prepare(requests) => engine.prepare(requests).map(Event::Plan),
                    Job::Execute(id) => engine
                        .execute_with_progress(&id, |done, total| {
                            let _ = events.send(Event::Progress(done, total));
                            ctx.request_repaint();
                        })
                        .map(Event::Plan),
                    Job::Undo(id) => engine.rollback(&id).map(Event::Plan),
                    Job::History => engine.history().map(Event::History),
                    Job::Preview(path) => {
                        read_preview(engine.scope(), &path).map(|text| Event::Preview(path, text))
                    }
                };
                let _ = events.send(result.unwrap_or_else(|e| Event::Error(format!("{e:#}"))));
                if let Ok(history) = engine.history() {
                    let _ = events.send(Event::History(history));
                }
                let _ = events.send(Event::Idle);
                ctx.request_repaint();
            }
        });
        let app = Self {
            scope,
            panes,
            active: 0,
            tx,
            rx,
            busy: false,
            progress: (0, 0),
            plan: None,
            history: vec![],
            activity: false,
            clipboard: None,
            message: "就绪".into(),
            zh: true,
            light: false,
            confirm_undo: None,
            preview: None,
            started: Instant::now(),
            smoke_seconds,
        };
        let _ = app.tx.send(Job::History);
        app
    }
    fn t<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        if self.zh { zh } else { en }
    }
    fn send(&mut self, job: Job) {
        if self.busy {
            return;
        }
        if self.tx.send(job).is_ok() {
            self.busy = true;
            self.progress = (0, 0);
            self.message = self.t("正在处理…", "Working…").into();
        }
    }
    fn prepare(&mut self, action: Action, sources: Vec<PathBuf>, target: usize) {
        let requests = sources
            .into_iter()
            .filter_map(|source| {
                let name = source.file_name()?.to_owned();
                Some(Request {
                    action,
                    source,
                    destination: self.panes[target].path.join(name),
                })
            })
            .collect::<Vec<_>>();
        if requests.is_empty() {
            self.message = self
                .t("请先选择普通文件", "Select regular files first")
                .into();
            return;
        }
        self.send(Job::Prepare(requests));
    }
    fn poll(&mut self, ctx: &egui::Context) {
        for pane in &mut self.panes {
            pane.poll();
        }
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Plan(plan) => {
                    self.message = format!(
                        "{} · {} / {}",
                        plan.id.get(..8).unwrap_or(&plan.id),
                        state_name(plan.state, self.zh),
                        plan.items.len()
                    );
                    if matches!(plan.state, PlanState::Completed | PlanState::RolledBack) {
                        for pane in &mut self.panes {
                            pane.refresh(&self.scope, ctx);
                        }
                    }
                    self.plan = Some(plan);
                }
                Event::History(history) => self.history = history,
                Event::Preview(path, text) => self.preview = Some((path, text)),
                Event::Progress(done, total) => self.progress = (done, total),
                Event::Error(error) => {
                    self.message = error;
                    self.plan = None;
                }
                Event::Idle => {
                    self.busy = false;
                    if matches!(self.message.as_str(), "正在处理…" | "Working…") {
                        self.message = self.t("就绪", "Ready").into();
                    }
                }
            }
        }
        if self.busy || self.panes.iter().any(|p| p.scanning) {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
    fn keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let a = self.active;
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::A)) {
            self.panes[a].selected = self.panes[a]
                .visible
                .iter()
                .map(|i| self.panes[a].entries[*i].path.clone())
                .collect();
        }
        // Winit translates native Cmd/Ctrl+C/X/V into clipboard events, not Key events.
        // Put useful path text on the system clipboard and require it to still match
        // before using the app's original paths. External text must not paste stale files.
        for command in clipboard_commands(ctx) {
            match command {
                ClipboardCommand::Copy | ClipboardCommand::Cut => {
                    let action = if command == ClipboardCommand::Copy {
                        Action::Copy
                    } else {
                        Action::Move
                    };
                    if let Some(clipboard) =
                        FileClipboard::new(action, self.panes[a].selected.iter().cloned().collect())
                    {
                        ctx.copy_text(clipboard.text.clone());
                        self.clipboard = Some(clipboard);
                        self.message = self
                            .t(
                                "已记住所选文件；粘贴时将先预演",
                                "Files selected for transfer; paste will open a preview",
                            )
                            .into();
                    }
                }
                ClipboardCommand::Paste(text) => {
                    if let Some(clipboard) = self.clipboard.clone()
                        && clipboard.matches(&text)
                    {
                        self.prepare(clipboard.action, clipboard.sources, a);
                    }
                }
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            self.panes[a].refresh(&self.scope, ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.plan = None;
            self.preview = None;
            self.confirm_undo = None;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
            && !self.panes[a].visible.is_empty()
        {
            let next = if self.panes[a].selected.is_empty() {
                0
            } else {
                (self.panes[a].anchor + 1).min(self.panes[a].visible.len() - 1)
            };
            self.panes[a].select(next, false, false);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
            && !self.panes[a].visible.is_empty()
        {
            let next = self.panes[a]
                .anchor
                .saturating_sub(1)
                .min(self.panes[a].visible.len() - 1);
            self.panes[a].select(next, false, false);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            && let Some(path) = self.panes[a].selected.first().cloned()
        {
            self.open_entry(a, path, ctx);
        }
    }
    fn open_entry(&mut self, pane: usize, path: PathBuf, ctx: &egui::Context) {
        if let Err(e) = self.scope.checked(&path, false) {
            self.message = e.to_string();
            return;
        }
        if path.is_dir() {
            self.panes[pane].navigate(&self.scope, path, ctx);
        } else {
            self.send(Job::Preview(path));
        }
    }
}

fn size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

fn read_preview(scope: &Scope, path: &Path) -> anyhow::Result<String> {
    use std::io::Read;
    scope.checked(path, false)?;
    anyhow::ensure!(
        std::fs::symlink_metadata(path)?.is_file(),
        "Only regular files can be previewed"
    );
    let mut bytes = vec![];
    std::fs::File::open(path)?
        .take(16 * 1024)
        .read_to_end(&mut bytes)?;
    if bytes.contains(&0) {
        return Ok("二进制文件 · 此原型提供文本预览 / Binary file · text preview only".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
fn state_name(state: PlanState, zh: bool) -> &'static str {
    match (state, zh) {
        (PlanState::Prepared, true) => "待确认",
        (PlanState::Running, true) => "执行中 / 待恢复",
        (PlanState::Completed, true) => "已完成",
        (PlanState::Failed, true) => "失败",
        (PlanState::PartiallyCompleted, true) => "部分完成",
        (PlanState::RollingBack, true) => "撤销中",
        (PlanState::RolledBack, true) => "已撤销",
        (PlanState::Prepared, false) => "Prepared",
        (PlanState::Running, false) => "Running / interrupted",
        (PlanState::Completed, false) => "Completed",
        (PlanState::Failed, false) => "Failed",
        (PlanState::PartiallyCompleted, false) => "Partially completed",
        (PlanState::RollingBack, false) => "Rolling back",
        (PlanState::RolledBack, false) => "Rolled back",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_clipboard_events_handle_files_and_reject_stale_external_text() {
        let ctx = egui::Context::default();
        let clipboard = FileClipboard::new(
            Action::Copy,
            vec![
                PathBuf::from("/fixture/中文 note.txt"),
                PathBuf::from("/fixture/b.txt"),
            ],
        )
        .unwrap();
        let mut commands = vec![];
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::Copy,
                    egui::Event::Cut,
                    egui::Event::Paste(clipboard.text.clone()),
                ],
                ..Default::default()
            },
            |_ui| {
                commands = clipboard_commands(&ctx);
            },
        );
        assert_eq!(
            commands,
            [
                ClipboardCommand::Copy,
                ClipboardCommand::Cut,
                ClipboardCommand::Paste(clipboard.text.clone())
            ]
        );
        assert!(clipboard.matches(&clipboard.text));
        assert!(!clipboard.matches("Unrelated external text"));
        assert_eq!(clipboard.sources[0], Path::new("/fixture/中文 note.txt"));
        assert!(FileClipboard::new(Action::Copy, vec![]).is_none());
        output.textures_delta.clear();
    }
    #[test]
    fn text_preview_is_bounded_and_respects_scope() {
        let fixture = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(fixture.path()).unwrap();
        let engine = Engine::open(&root.join("state"), std::slice::from_ref(&root)).unwrap();
        let path = root.join("large.txt");
        std::fs::write(&path, vec![b'a'; 100_000]).unwrap();
        assert_eq!(
            read_preview(engine.scope(), &path).unwrap().len(),
            16 * 1024
        );
        std::fs::write(root.join("binary"), [0, 1, 2]).unwrap();
        assert!(
            read_preview(engine.scope(), &root.join("binary"))
                .unwrap()
                .contains("Binary file")
        );
        assert!(read_preview(engine.scope(), &root.join("state/operations.db")).is_err());
    }
    #[test]
    fn virtual_list_only_builds_visible_rows_at_one_hundred_thousand() {
        let ctx = egui::Context::default();
        let mut max_rows = 0;
        let started = Instant::now();
        for _ in 0..20 {
            let mut count = 0;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 700.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        egui::ScrollArea::vertical().show_rows(ui, 29.0, 100_000, |ui, rows| {
                            for row in rows {
                                count += 1;
                                ui.selectable_label(false, format!("File {row:08}.txt"))
                                    .on_hover_text("Full path");
                            }
                        });
                    });
                },
            );
            assert!(!output.shapes.is_empty());
            max_rows = max_rows.max(count);
            output.textures_delta.clear();
        }
        assert!(max_rows < 50, "Virtualization built {max_rows} rows");
        println!(
            "virtual_rows total=100000 built_max={max_rows} frames=20 elapsed_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
    #[test]
    fn texture_custom_drawing_and_high_dpi_are_supported() {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(2.0);
        let texture = ctx.load_texture(
            "thumbnail-probe",
            egui::ColorImage::filled([32, 32], theme::Palette::new(false).accent),
            egui::TextureOptions::LINEAR,
        );
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.image(&texture);
                ui.painter().circle_filled(
                    egui::pos2(80.0, 80.0),
                    10.0,
                    theme::Palette::new(false).accent,
                );
            });
        });
        assert!(!output.textures_delta.set.is_empty());
        assert!(!output.shapes.is_empty());
        assert_eq!(ctx.pixels_per_point(), 2.0);
        output.textures_delta.clear();
    }
    #[test]
    fn multiselection_and_filtering_keep_original_paths() {
        let mut pane = Pane::new(PathBuf::from("/fixture"));
        for i in 0..10 {
            pane.entries.push(Entry {
                path: PathBuf::from(format!("/fixture/{i}")),
                name: format!("file-{i}"),
                directory: false,
                link: false,
                bytes: 0,
                modified: None,
                error: None,
            });
        }
        pane.filter();
        pane.select(2, false, false);
        pane.select(5, false, true);
        assert_eq!(pane.selected.len(), 4);
        pane.select(4, true, false);
        assert_eq!(pane.selected.len(), 3);
        pane.filter = "file-9".into();
        pane.filter();
        assert_eq!(pane.visible, [9]);
        pane.select(0, false, false);
        assert!(pane.selected.contains(Path::new("/fixture/9")));
    }
}
