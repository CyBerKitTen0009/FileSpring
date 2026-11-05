use eframe::egui::{self, Color32, RichText};
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

const ACCENT: Color32 = Color32::from_rgb(112, 220, 191);
const MUTED: Color32 = Color32::from_rgb(147, 158, 174);

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
    clipboard: Option<(Action, Vec<PathBuf>)>,
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
        ctx.set_theme(egui::Theme::Dark);
        let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Color32::from_rgb(22, 27, 35);
        style.visuals.window_fill = Color32::from_rgb(28, 34, 44);
        style.visuals.selection.bg_fill = Color32::from_rgb(42, 82, 78);
        style.visuals.selection.stroke.color = ACCENT;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        ctx.set_style_of(egui::Theme::Dark, style);
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
            message: "就绪 · 文件更改前请先预演 / Ready · Preview before changing files".into(),
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
                Event::Idle => self.busy = false,
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
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)) {
            self.clipboard = Some((
                Action::Copy,
                self.panes[a].selected.iter().cloned().collect(),
            ));
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::X)) {
            self.clipboard = Some((
                Action::Move,
                self.panes[a].selected.iter().cloned().collect(),
            ));
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::V))
            && let Some((action, sources)) = self.clipboard.clone()
        {
            self.prepare(action, sources, a);
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
    fn pane(&mut self, ui: &mut egui::Ui, index: usize) {
        let ctx = ui.ctx().clone();
        let zh = self.zh;
        let title = if zh {
            if index == 0 {
                "左侧文件夹"
            } else {
                "右侧文件夹"
            }
        } else if index == 0 {
            "LEFT FOLDER"
        } else {
            "RIGHT FOLDER"
        };
        let mut navigate = None;
        let mut going_back = false;
        let mut open = None;
        let mut context_copy = false;
        let inner = egui::Frame::group(ui.style())
            .inner_margin(14)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(title)
                            .strong()
                            .color(if self.active == index { ACCENT } else { MUTED }),
                    );
                    if ui.small_button(if zh { "激活" } else { "Focus" }).clicked() {
                        self.active = index;
                    }
                    if ui.small_button("↻").on_hover_text("F5").clicked() {
                        self.panes[index].refresh(&self.scope, &ctx);
                    }
                    if ui
                        .small_button("↑")
                        .on_hover_text(if zh { "上一级" } else { "Parent" })
                        .clicked()
                    {
                        navigate = self.panes[index].path.parent().map(Path::to_path_buf);
                    }
                    if ui.small_button("←").clicked() {
                        navigate = self.panes[index].back.pop();
                        going_back = true;
                    }
                });
                let path = ui.add(
                    egui::TextEdit::singleline(&mut self.panes[index].input)
                        .id_salt(("path", index))
                        .desired_width(f32::INFINITY),
                );
                if path.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    navigate = Some(PathBuf::from(&self.panes[index].input));
                }
                if self.active == index
                    && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::L))
                {
                    path.request_focus();
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.panes[index].filter)
                            .hint_text(if zh {
                                "筛选当前目录…"
                            } else {
                                "Filter this folder…"
                            })
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    self.panes[index].filter();
                }
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if zh { "名称" } else { "NAME" })
                            .small()
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(if zh { "大小" } else { "SIZE" })
                                .small()
                                .color(MUTED),
                        );
                    });
                });
                ui.separator();
                if let Some(error) = &self.panes[index].error {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
                let height = (ui.available_height() - 32.0).max(120.0);
                egui::ScrollArea::vertical()
                    .id_salt(("files", index))
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, false])
                    .show_rows(ui, 29.0, self.panes[index].visible.len(), |ui, rows| {
                        for row in rows {
                            let entry =
                                self.panes[index].entries[self.panes[index].visible[row]].clone();
                            let selected = self.panes[index].selected.contains(&entry.path);
                            let icon = if entry.directory {
                                "▸"
                            } else if entry.link {
                                "↗"
                            } else {
                                "·"
                            };
                            let response = ui
                                .horizontal(|ui| {
                                    let width = (ui.available_width() - 90.0).max(80.0);
                                    let response = ui.add_sized(
                                        [width, 29.0],
                                        egui::Button::new("")
                                            .left_text(
                                                RichText::new(format!("{icon}  {}", entry.name))
                                                    .color(if entry.directory {
                                                        ACCENT
                                                    } else {
                                                        ui.visuals().text_color()
                                                    }),
                                            )
                                            .selected(selected)
                                            .frame(selected)
                                            .truncate(),
                                    );
                                    ui.label(
                                        RichText::new(if entry.directory {
                                            "—".into()
                                        } else {
                                            size(entry.bytes)
                                        })
                                        .small()
                                        .color(MUTED),
                                    );
                                    response
                                })
                                .inner;
                            if response.clicked() {
                                self.active = index;
                                let modifiers = ui.input(|i| i.modifiers);
                                self.panes[index].select(
                                    row,
                                    modifiers.command || modifiers.ctrl,
                                    modifiers.shift,
                                );
                            }
                            if response.double_clicked() {
                                open = Some(entry.path.clone());
                            }
                            response.dnd_set_drag_payload(vec![entry.path.clone()]);
                            response.clone().on_hover_text(format!(
                                "{}{}",
                                entry.path.display(),
                                entry.error.map(|s| format!("\n{s}")).unwrap_or_default()
                            ));
                            response.context_menu(|ui| {
                                if ui
                                    .button(if zh {
                                        "查看 / 打开"
                                    } else {
                                        "Preview / open"
                                    })
                                    .clicked()
                                {
                                    open = Some(entry.path.clone());
                                    ui.close();
                                }
                                if ui
                                    .button(if zh { "复制路径" } else { "Copy path" })
                                    .clicked()
                                {
                                    ui.ctx().copy_text(entry.path.display().to_string());
                                    ui.close();
                                }
                                if !entry.directory
                                    && !entry.link
                                    && ui
                                        .button(if zh {
                                            "预演复制到另一栏"
                                        } else {
                                            "Preview copy to other pane"
                                        })
                                        .clicked()
                                {
                                    self.panes[index].selected.insert(entry.path.clone());
                                    context_copy = true;
                                    ui.close();
                                }
                            });
                        }
                    });
                if self.panes[index].scanning {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(if zh { "正在读取…" } else { "Reading…" });
                    });
                } else if self.panes[index].visible.is_empty() {
                    ui.label(
                        RichText::new(if zh {
                            "没有匹配的文件"
                        } else {
                            "No matching files"
                        })
                        .color(MUTED),
                    );
                }
                ui.label(
                    RichText::new(format!(
                        "{} {} · {} {}",
                        self.panes[index].entries.len(),
                        if zh { "项" } else { "items" },
                        self.panes[index].selected.len(),
                        if zh { "已选" } else { "selected" }
                    ))
                    .small()
                    .color(MUTED),
                );
            });
        if inner.response.contains_pointer() && ui.input(|i| i.pointer.any_pressed()) {
            self.active = index;
        }
        if let Some(paths) = inner.response.dnd_release_payload::<Vec<PathBuf>>() {
            self.prepare(Action::Copy, paths.as_ref().clone(), index);
        }
        if let Some(path) = navigate {
            let before = self.panes[index].back.len();
            self.panes[index].navigate(&self.scope, path, &ctx);
            if going_back && self.panes[index].back.len() > before {
                self.panes[index].back.pop();
            }
        }
        if let Some(path) = open {
            self.open_entry(index, path, &ctx);
        }
        if context_copy {
            self.prepare(
                Action::Copy,
                self.panes[index].selected.iter().cloned().collect(),
                1 - index,
            );
        }
    }

    fn plan_view(&mut self, ui: &mut egui::Ui) {
        let Some(plan) = self.plan.clone() else {
            return;
        };
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.t("操作计划", "Operation plan"))
                    .size(18.0)
                    .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "{} · {} · {}",
                    state_name(plan.state, self.zh),
                    plan.items.len(),
                    size(plan.bytes())
                ))
                .color(MUTED),
            );
            if ui.small_button(self.t("关闭", "Close")).clicked() {
                self.plan = None;
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("plan")
            .max_height(140.0)
            .show_rows(ui, 26.0, plan.items.len(), |ui, rows| {
                for i in rows {
                    let item = &plan.items[i];
                    ui.label(format!(
                        "{:?}  {}  →  {}",
                        item.request.action,
                        item.request.source.display(),
                        item.request.destination.display()
                    ));
                }
            });
        for conflict in &plan.conflicts {
            ui.colored_label(Color32::LIGHT_RED, conflict);
        }
        if let Some(error) = &plan.error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        if plan.state == PlanState::Prepared {
            ui.horizontal(|ui| {
                ui.label(self.t(
                    "仅普通文件；不覆盖；复制后校验；移动可撤销。",
                    "Regular files only · No overwrite · Verified copy · Undo available",
                ));
                if ui
                    .add_enabled(
                        !self.busy && plan.executable(),
                        egui::Button::new(self.t("确认执行", "Confirm & execute"))
                            .fill(Color32::from_rgb(38, 103, 87)),
                    )
                    .clicked()
                {
                    self.send(Job::Execute(plan.id));
                }
            });
        }
    }
    fn activity_view(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("操作历史与恢复", "Activity & recovery"));
        ui.label(
            RichText::new(self.t(
                "中断的操作需要手动继续。撤销历史不等同于备份。",
                "Interrupted operations require explicit resume. Undo history is not a backup.",
            ))
            .color(MUTED),
        );
        let history = self.history.clone();
        egui::ScrollArea::vertical()
            .max_height(
                (ui.available_height() - if self.plan.is_some() { 270.0 } else { 90.0 }).max(150.0),
            )
            .show(ui, |ui| {
                for plan in history {
                    egui::Frame::group(ui.style())
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(state_name(plan.state, self.zh)).strong());
                                ui.label(format!(
                                    "{} · {} · {}",
                                    &plan.id[..8],
                                    plan.items.len(),
                                    size(plan.bytes())
                                ));
                                if ui.button(self.t("查看", "Review")).clicked() {
                                    self.plan = Some(plan.clone());
                                }
                                if !matches!(
                                    plan.state,
                                    PlanState::Prepared
                                        | PlanState::Completed
                                        | PlanState::RolledBack
                                ) && ui
                                    .add_enabled(
                                        !self.busy,
                                        egui::Button::new(self.t("继续", "Resume")),
                                    )
                                    .clicked()
                                {
                                    if plan.rollback_requested {
                                        self.confirm_undo = Some(plan.id.clone());
                                    } else {
                                        self.plan = Some(plan.clone());
                                    }
                                }
                                if matches!(
                                    plan.state,
                                    PlanState::Completed | PlanState::PartiallyCompleted
                                ) && ui
                                    .add_enabled(
                                        !self.busy,
                                        egui::Button::new(self.t("撤销…", "Undo…")),
                                    )
                                    .clicked()
                                {
                                    self.confirm_undo = Some(plan.id.clone());
                                }
                            });
                            if let Some(error) = plan.error {
                                ui.colored_label(Color32::LIGHT_RED, error);
                            }
                        });
                }
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.keys(&ctx);
        if let Some(seconds) = self.smoke_seconds {
            if self.started.elapsed().as_secs() >= seconds {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        if self.busy && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.message = self
                .t(
                    "任务正在执行，请等待任务完成。",
                    "Please wait for the current operation to finish.",
                )
                .into();
        }
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if !dropped.is_empty() && !self.busy {
            self.prepare(Action::Copy, dropped, self.active);
        }
        egui::Panel::left("sidebar")
            .exact_size(170.0)
            .show(ui, |ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("FileSpring")
                        .size(24.0)
                        .strong()
                        .color(ACCENT),
                );
                ui.label(RichText::new("FILE WORKSPACE").size(10.0).color(MUTED));
                ui.add_space(24.0);
                if ui
                    .selectable_label(!self.activity, self.t("▣  文件", "▣  Files"))
                    .clicked()
                {
                    self.activity = false;
                }
                if ui
                    .selectable_label(self.activity, self.t("↶  操作历史", "↶  Activity"))
                    .clicked()
                {
                    self.activity = true;
                    self.send(Job::History);
                }
                ui.add_space(22.0);
                ui.label(
                    RichText::new(self.t("已授权目录", "AUTHORIZED ROOTS"))
                        .small()
                        .color(MUTED),
                );
                for root in self.scope.roots().to_vec() {
                    let name = root
                        .file_name()
                        .unwrap_or(root.as_os_str())
                        .to_string_lossy();
                    if ui
                        .button(format!("▸ {name}"))
                        .on_hover_text(root.display().to_string())
                        .clicked()
                    {
                        self.panes[self.active].navigate(&self.scope, root, &ctx);
                        self.activity = false;
                    }
                }
                ui.add_space(20.0);
                if ui.button(self.t("选择目录…", "Choose folder…")).clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.panes[self.active].path)
                        .pick_folder()
                {
                    self.panes[self.active].navigate(&self.scope, path, &ctx);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("0.1.0 · Phase 1 prototype")
                            .small()
                            .color(MUTED),
                    );
                    if ui
                        .button(if self.zh { "English" } else { "简体中文" })
                        .clicked()
                    {
                        self.zh = !self.zh;
                    }
                    if ui.button(self.t("切换主题", "Toggle theme")).clicked() {
                        self.light = !self.light;
                        ctx.set_visuals(if self.light {
                            egui::Visuals::light()
                        } else {
                            egui::Visuals::dark()
                        });
                    }
                });
            });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.busy {
                    ui.spinner();
                }
                ui.label(&self.message);
                if self.progress.1 > 0 {
                    ui.add(
                        egui::ProgressBar::new(self.progress.0 as f32 / self.progress.1 as f32)
                            .desired_width(130.0)
                            .show_percentage(),
                    );
                }
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(self.t("文件", "Files"));
                ui.label(RichText::new(self.t("浏览 · 预演 · 安全执行", "Browse · Review · Execute safely")).color(MUTED));
            });
            ui.add_space(8.0);
            if self.activity { self.activity_view(ui); }
            else {
                ui.horizontal(|ui| {
                    let sources = self.panes[self.active].selected.iter().cloned().collect::<Vec<_>>();
                    let target = 1-self.active;
                    if ui.add_enabled(!self.busy && !sources.is_empty(), egui::Button::new(self.t("复制到另一栏 →", "Copy to other pane →"))).clicked() { self.prepare(Action::Copy, sources.clone(), target); }
                    if ui.add_enabled(!self.busy && !sources.is_empty(), egui::Button::new(self.t("移动到另一栏 →", "Move to other pane →"))).clicked() { self.prepare(Action::Move, sources, target); }
                    ui.label(RichText::new(self.t("先预演，再确认", "Preview, then confirm")).small().color(MUTED));
                });
                ui.add_space(8.0);
                let reserve = if self.plan.is_some() { 265.0 } else { 0.0 };
                ui.allocate_ui(egui::vec2(ui.available_width(), (ui.available_height()-reserve).max(200.0)), |ui| {
                    ui.columns(2, |columns| { self.pane(&mut columns[0], 0); self.pane(&mut columns[1], 1); });
                });
            }
            if self.activity
                && let Some(plan) = self.plan.clone()
                && !matches!(plan.state, PlanState::Prepared | PlanState::Completed | PlanState::RolledBack)
                && !plan.rollback_requested
                && ui.add_enabled(!self.busy, egui::Button::new(self.t("确认继续此计划", "Confirm resume of this plan"))).clicked() {
                self.send(Job::Execute(plan.id));
            }
            self.plan_view(ui);
            if let Some(id) = self.confirm_undo.clone() {
                ui.separator(); ui.label(self.t("确认撤销？只有内容与记录一致的文件才会被改动；原路径被占用时会停止。", "Undo this plan? Only unchanged recorded files are modified. Occupied original paths stop recovery."));
                if ui.add_enabled(!self.busy, egui::Button::new(self.t("确认撤销", "Confirm undo"))).clicked() { self.confirm_undo = None; self.send(Job::Undo(id)); }
                if ui.button(self.t("取消", "Cancel")).clicked() { self.confirm_undo = None; }
            }
        });
        if let Some((path, text)) = &self.preview {
            let mut open = true;
            egui::Window::new(
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy(),
            )
            .open(&mut open)
            .default_size([640.0, 420.0])
            .show(&ctx, |ui| {
                ui.label(
                    RichText::new(path.display().to_string())
                        .small()
                        .color(MUTED),
                );
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.monospace(text);
                });
            });
            if !open {
                self.preview = None;
            }
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
            egui::ColorImage::filled([32, 32], ACCENT),
            egui::TextureOptions::LINEAR,
        );
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.image(&texture);
                ui.painter()
                    .circle_filled(egui::pos2(80.0, 80.0), 10.0, ACCENT);
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
