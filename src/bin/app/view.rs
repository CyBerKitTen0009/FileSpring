use super::{
    icons::{self, Icon},
    theme::Palette,
    *,
};
use egui::{Align, Color32, RichText, Stroke};

impl App {
    fn sidebar_item(
        &self,
        ui: &mut egui::Ui,
        icon: Icon,
        title: &str,
        selected: bool,
    ) -> egui::Response {
        let p = Palette::new(self.light);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.), egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, title)
        });
        let color = if selected { p.selected_text } else { p.text };
        if selected || response.hovered() {
            ui.painter()
                .rect_filled(rect, 6, if selected { p.selection } else { p.line });
        }
        icons::paint(
            ui.painter(),
            egui::Rect::from_center_size(
                rect.left_center() + egui::vec2(20., 0.),
                egui::vec2(18., 18.),
            ),
            icon,
            if selected { color } else { p.accent },
        );
        let painter = ui
            .painter()
            .with_clip_rect(rect.shrink2(egui::vec2(8., 0.)));
        icons::text(
            &painter,
            rect.left_center() + egui::vec2(38., 0.),
            title,
            13.,
            color,
        );
        response
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = Palette::new(self.light);
        egui::Panel::left("sidebar")
            .exact_size(196.)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(p.sidebar)
                    .inner_margin(14)
                    .stroke(Stroke::new(1., p.line)),
            )
            .show(ui, |ui| {
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(32., 32.), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 8, p.selection);
                    icons::paint(ui.painter(), rect.shrink(5.), Icon::Folder, Color32::WHITE);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("FileSpring").size(17.).strong());
                        ui.label(
                            RichText::new(self.t("文件，井然有序", "A place for your files"))
                                .size(10.)
                                .color(p.secondary),
                        );
                    });
                });
                ui.add_space(28.);
                ui.label(
                    RichText::new(self.t("工作空间", "Workspace"))
                        .small()
                        .strong()
                        .color(p.secondary),
                );
                if self
                    .sidebar_item(ui, Icon::Grid, self.t("所有文件", "Files"), !self.activity)
                    .clicked()
                {
                    self.activity = false;
                }
                if self
                    .sidebar_item(
                        ui,
                        Icon::History,
                        self.t("操作历史", "Activity"),
                        self.activity,
                    )
                    .clicked()
                {
                    self.activity = true;
                }
                ui.add_space(24.);
                ui.label(
                    RichText::new(self.t("工作目录", "Locations"))
                        .small()
                        .strong()
                        .color(p.secondary),
                );
                for root in self.scope.roots().to_vec() {
                    let name = folder_name(&root);
                    if self
                        .sidebar_item(ui, Icon::Folder, &name, false)
                        .on_hover_text(root.display().to_string())
                        .clicked()
                    {
                        self.panes[self.active].navigate(&self.scope, root, ui.ctx());
                        self.activity = false;
                    }
                }
                ui.add_space(6.);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(self.t("选择目录…", "Choose folder…")).color(p.secondary),
                        )
                        .frame(false),
                    )
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.panes[self.active].path)
                        .pick_folder()
                {
                    self.panes[self.active].navigate(&self.scope, path, ui.ctx());
                    self.activity = false;
                }
                ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(RichText::new("0.1 · Preview").small().color(p.secondary));
                    ui.add_space(6.);
                    ui.horizontal(|ui| {
                        if icons::button(
                            ui,
                            if self.light { Icon::Moon } else { Icon::Sun },
                            self.t("切换主题", "Toggle theme"),
                            true,
                            p.secondary,
                        )
                        .clicked()
                        {
                            self.light = !self.light;
                            ui.ctx().set_theme(if self.light {
                                egui::Theme::Light
                            } else {
                                egui::Theme::Dark
                            });
                        }
                        if ui
                            .add(
                                egui::Button::new(if self.zh { "English" } else { "简体中文" })
                                    .frame(false),
                            )
                            .clicked()
                        {
                            self.zh = !self.zh;
                        }
                    });
                    ui.separator();
                });
            });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let p = Palette::new(self.light);
        egui::Panel::top("toolbar")
            .exact_size(76.)
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(22, 14)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(if self.activity {
                                self.t("操作历史", "Activity")
                            } else {
                                self.t("文件", "Files")
                            })
                            .size(21.)
                            .strong(),
                        );
                        ui.label(
                            RichText::new(if self.activity {
                                self.t("每一次改动，都有记录。", "Every change, accounted for.")
                                    .to_owned()
                            } else {
                                format!(
                                    "{}  /  {}",
                                    folder_name(&self.panes[0].path),
                                    folder_name(&self.panes[1].path)
                                )
                            })
                            .size(11.)
                            .color(p.secondary),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if !self.activity {
                            let paths: Vec<_> =
                                self.panes[self.active].selected.iter().cloned().collect();
                            let enabled = !self.busy && !paths.is_empty();
                            if ui
                                .add_enabled(
                                    enabled,
                                    egui::Button::new(self.t("移动…", "Move…"))
                                        .min_size(egui::vec2(68., 30.)),
                                )
                                .on_hover_text(
                                    self.t("预演移动到另一栏", "Preview move to other pane"),
                                )
                                .clicked()
                            {
                                self.prepare(Action::Move, paths.clone(), 1 - self.active);
                            }
                            if ui
                                .add_enabled(
                                    enabled,
                                    egui::Button::new(self.t("复制…", "Copy…"))
                                        .min_size(egui::vec2(68., 30.)),
                                )
                                .on_hover_text(
                                    self.t("预演复制到另一栏", "Preview copy to other pane"),
                                )
                                .clicked()
                            {
                                self.prepare(Action::Copy, paths, 1 - self.active);
                            }
                            ui.label(
                                RichText::new(self.t("先预演，再确认", "Review before applying"))
                                    .small()
                                    .color(p.secondary),
                            );
                        } else if icons::button(
                            ui,
                            Icon::Refresh,
                            self.t("刷新历史", "Refresh activity"),
                            !self.busy,
                            p.secondary,
                        )
                        .clicked()
                        {
                            self.send(Job::History);
                        }
                    });
                });
            });
    }

    fn pane(&mut self, ui: &mut egui::Ui, index: usize) {
        let p = Palette::new(self.light);
        let ctx = ui.ctx().clone();
        let zh = self.zh;
        let active = self.active == index;
        let mut navigate = None;
        let mut going_back = false;
        let mut open = None;
        let mut context_copy = false;
        let frame = egui::Frame::new()
            .fill(p.background)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if icons::button(
                        ui,
                        Icon::Back,
                        if zh { "返回" } else { "Back" },
                        !self.panes[index].back.is_empty(),
                        p.secondary,
                    )
                    .clicked()
                    {
                        navigate = self.panes[index].back.pop();
                        going_back = true;
                    }
                    let parent = self.panes[index].path.parent().map(Path::to_path_buf);
                    let can_parent = parent
                        .as_ref()
                        .is_some_and(|p| self.scope.checked(p, false).is_ok());
                    if icons::button(
                        ui,
                        Icon::Up,
                        if zh { "上一级" } else { "Parent" },
                        can_parent,
                        p.secondary,
                    )
                    .clicked()
                    {
                        navigate = parent;
                    }
                    ui.label(
                        RichText::new(folder_name(&self.panes[index].path))
                            .size(14.)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if icons::button(
                            ui,
                            Icon::Refresh,
                            if zh { "刷新 · F5" } else { "Refresh · F5" },
                            !self.panes[index].scanning,
                            p.secondary,
                        )
                        .clicked()
                        {
                            self.panes[index].refresh(&self.scope, &ctx);
                        }
                        if active {
                            ui.label(
                                RichText::new(if zh { "当前" } else { "Active" })
                                    .size(10.)
                                    .color(p.accent),
                            );
                        }
                    });
                });
                let path = ui.add(
                    egui::TextEdit::singleline(&mut self.panes[index].input)
                        .id_salt(("path", index))
                        .font(egui::FontId::proportional(11.))
                        .margin(egui::vec2(9., 6.))
                        .desired_width(f32::INFINITY),
                );
                if path.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    navigate = Some(PathBuf::from(&self.panes[index].input));
                }
                if active
                    && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::L))
                {
                    path.request_focus();
                }
                egui::Frame::new()
                    .fill(p.subtle)
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            icons::label(ui, Icon::Search, p.secondary);
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.panes[index].filter)
                                        .id_salt(("filter", index))
                                        .hint_text(if zh {
                                            "筛选此文件夹"
                                        } else {
                                            "Filter this folder"
                                        })
                                        .frame(egui::Frame::NONE)
                                        .desired_width(f32::INFINITY),
                                )
                                .changed()
                            {
                                self.panes[index].filter();
                            }
                        });
                    });
                ui.add_space(6.);
                let (header, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 25.),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(header, 4, p.subtle);
                icons::text(
                    ui.painter(),
                    header.left_center() + egui::vec2(10., 0.),
                    if zh { "名称" } else { "Name" },
                    11.,
                    p.secondary,
                );
                ui.painter().text(
                    header.right_center() - egui::vec2(12., 0.),
                    egui::Align2::RIGHT_CENTER,
                    if zh { "大小" } else { "Size" },
                    egui::FontId::proportional(11.),
                    p.secondary,
                );
                let height = (ui.available_height() - 28.).max(60.);
                let count = self.panes[index].visible.len();
                let empty_text = if self.panes[index].scanning {
                    if zh { "正在读取…" } else { "Reading…" }
                } else if self.panes[index].filter.is_empty() {
                    if zh {
                        "此文件夹为空"
                    } else {
                        "This folder is empty"
                    }
                } else if zh {
                    "没有匹配的文件"
                } else {
                    "No matching files"
                };
                ui.spacing_mut().item_spacing.y = 0.;
                let area = egui::ScrollArea::vertical()
                    .id_salt(("files", index))
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, false])
                    .show_rows(ui, 32., count, |ui, rows| {
                        ui.spacing_mut().item_spacing.y = 0.;
                        for row in rows {
                            let entry =
                                self.panes[index].entries[self.panes[index].visible[row]].clone();
                            let selected = self.panes[index].selected.contains(&entry.path);
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 32.),
                                egui::Sense::click_and_drag(),
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::Role::Button,
                                    ui.is_enabled(),
                                    selected,
                                    format!(
                                        "{} · {}",
                                        entry.name,
                                        if entry.directory {
                                            self.t("文件夹", "Folder").into()
                                        } else {
                                            size(entry.bytes)
                                        }
                                    ),
                                )
                            });
                            let fill = if selected {
                                if active { p.selection } else { p.line }
                            } else if response.hovered() || row % 2 == 1 {
                                p.subtle
                            } else {
                                p.background
                            };
                            ui.painter().rect_filled(rect, 4, fill);
                            let text_color = if selected && active {
                                p.selected_text
                            } else {
                                p.text
                            };
                            icons::paint(
                                ui.painter(),
                                egui::Rect::from_center_size(
                                    rect.left_center() + egui::vec2(18., 0.),
                                    egui::vec2(19., 19.),
                                ),
                                if entry.directory {
                                    Icon::Folder
                                } else if entry.link {
                                    Icon::Link
                                } else {
                                    Icon::File
                                },
                                if selected && active {
                                    text_color
                                } else if entry.directory {
                                    p.accent
                                } else {
                                    p.secondary
                                },
                            );
                            let name_rect = egui::Rect::from_min_max(
                                rect.min + egui::vec2(36., 0.),
                                rect.max - egui::vec2(82., 0.),
                            );
                            icons::text(
                                &ui.painter().with_clip_rect(name_rect),
                                name_rect.left_center(),
                                &entry.name,
                                13.,
                                text_color,
                            );
                            ui.painter().text(
                                rect.right_center() - egui::vec2(10., 0.),
                                egui::Align2::RIGHT_CENTER,
                                if entry.directory {
                                    "—".into()
                                } else {
                                    size(entry.bytes)
                                },
                                egui::FontId::proportional(11.),
                                if selected && active {
                                    text_color
                                } else {
                                    p.secondary
                                },
                            );
                            if response.clicked() {
                                self.active = index;
                                let m = ui.input(|i| i.modifiers);
                                self.panes[index].select(row, m.command || m.ctrl, m.shift);
                            }
                            if response.double_clicked() {
                                open = Some(entry.path.clone());
                            }
                            response.dnd_set_drag_payload(if selected {
                                self.panes[index]
                                    .selected
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>()
                            } else {
                                vec![entry.path.clone()]
                            });
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
                if count == 0 {
                    icons::text(
                        ui.painter(),
                        area.inner_rect.center() - egui::vec2(65., 0.),
                        empty_text,
                        13.,
                        p.secondary,
                    );
                }
                ui.spacing_mut().item_spacing.y = 6.;
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(item_count(self.panes[index].entries.len(), zh, false))
                            .small()
                            .color(p.secondary),
                    );
                    if self.panes[index].scanning {
                        ui.add(egui::Spinner::new().size(12.));
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} {}",
                                self.panes[index].selected.len(),
                                if zh { "项已选择" } else { "selected" }
                            ))
                            .small()
                            .color(p.secondary),
                        );
                    });
                });
            });
        if let Some(error) = self.panes[index].error.clone() {
            let mut visible = true;
            egui::Window::new(if zh {
                "无法打开目录"
            } else {
                "Cannot open folder"
            })
            .id(egui::Id::unique(("pane_error", index)))
            .open(&mut visible)
            .collapsible(false)
            .show(&ctx, |ui| {
                ui.label(error);
            });
            if !visible {
                self.panes[index].error = None;
            }
        }
        let inside = ctx.rect_contains_pointer(frame.response.layer_id, frame.response.rect);
        if inside && ui.input(|i| i.pointer.any_pressed()) {
            self.active = index;
        }
        if inside
            && ui.input(|i| i.pointer.any_released())
            && let Some(paths) = egui::DragAndDrop::take_payload::<Vec<PathBuf>>(&ctx)
        {
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
        let p = Palette::new(self.light);
        let interrupted = !matches!(
            plan.state,
            PlanState::Prepared | PlanState::Completed | PlanState::RolledBack
        );
        ui.horizontal(|ui| {
            icons::label(
                ui,
                if plan.state == PlanState::Completed {
                    Icon::Check
                } else {
                    Icon::History
                },
                p.accent,
            );
            ui.label(
                RichText::new(self.t("操作预演", "Review changes"))
                    .size(15.)
                    .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "{} · {} · {}",
                    state_name(plan.state, self.zh),
                    item_count(plan.items.len(), self.zh, true),
                    size(plan.bytes())
                ))
                .small()
                .color(p.secondary),
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if icons::button(
                    ui,
                    Icon::Close,
                    self.t("关闭预演", "Close review"),
                    !self.busy,
                    p.secondary,
                )
                .clicked()
                {
                    self.plan = None;
                }
                if plan.state == PlanState::Prepared
                    && ui
                        .add_enabled(
                            !self.busy && plan.executable(),
                            egui::Button::new(
                                RichText::new(self.t("确认执行", "Confirm & execute"))
                                    .color(Color32::WHITE),
                            )
                            .fill(p.selection),
                        )
                        .clicked()
                {
                    self.send(Job::Execute(plan.id.clone()));
                }
                if interrupted
                    && !plan.rollback_requested
                    && ui
                        .add_enabled(
                            !self.busy,
                            egui::Button::new(self.t("确认继续", "Confirm resume")),
                        )
                        .clicked()
                {
                    self.send(Job::Execute(plan.id.clone()));
                }
                if matches!(
                    plan.state,
                    PlanState::Completed | PlanState::PartiallyCompleted
                ) && ui
                    .add_enabled(!self.busy, egui::Button::new(self.t("撤销…", "Undo…")))
                    .clicked()
                {
                    self.confirm_undo = Some(plan.id.clone());
                }
            });
        });
        ui.label(RichText::new(self.t("不会覆盖已有文件。移动会先复制并校验，再移除源文件。", "Existing files are never overwritten. Moves copy and verify before removing the source.")).size(11.).color(p.secondary));
        ui.separator();
        if !plan.conflicts.is_empty() || plan.error.is_some() {
            egui::ScrollArea::vertical()
                .id_salt("plan_errors")
                .max_height(48.)
                .show(ui, |ui| {
                    for conflict in &plan.conflicts {
                        ui.colored_label(ui.visuals().error_fg_color, conflict);
                    }
                    if let Some(error) = &plan.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                });
        }
        ui.spacing_mut().item_spacing.y = 0.;
        egui::ScrollArea::vertical()
            .id_salt("plan")
            .max_height(ui.available_height().max(24.))
            .auto_shrink([false, false])
            .show_rows(ui, 48., plan.items.len(), |ui, rows| {
                for i in rows {
                    let item = &plan.items[i];
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 48.),
                        egui::Sense::hover(),
                    );
                    if i % 2 == 1 {
                        ui.painter().rect_filled(rect, 4, p.subtle);
                    }
                    icons::paint(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            rect.left_center() + egui::vec2(17., 0.),
                            egui::vec2(18., 18.),
                        ),
                        Icon::File,
                        p.secondary,
                    );
                    let action = match item.request.action {
                        Action::Copy => self.t("复制", "Copy"),
                        Action::Move => self.t("移动", "Move"),
                    };
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Label,
                            true,
                            format!(
                                "{action}: {} → {}",
                                item.request.source.display(),
                                item.request.destination.display()
                            ),
                        )
                    });
                    let left = rect.left_top() + egui::vec2(34., 13.);
                    let clip = ui.painter().with_clip_rect(rect.shrink(6.));
                    icons::text(
                        &clip,
                        left,
                        &format!("{}  {}", action, folder_name(&item.request.source)),
                        12.,
                        p.text,
                    );
                    icons::text(
                        &clip,
                        left + egui::vec2(0., 19.),
                        &format!(
                            "{}  →  {}",
                            item.request.source.display(),
                            item.request.destination.display()
                        ),
                        10.,
                        p.secondary,
                    );
                    response.on_hover_text(format!(
                        "{}\n→ {}",
                        item.request.source.display(),
                        item.request.destination.display()
                    ));
                }
            });
    }

    fn activity_view(&mut self, ui: &mut egui::Ui) {
        let p = Palette::new(self.light);
        ui.add_space(6.);
        ui.label(
            RichText::new(self.t(
                "中断的操作需要手动继续。撤销历史不等同于备份。",
                "Interrupted operations require explicit resume. Undo history is not a backup.",
            ))
            .small()
            .color(p.secondary),
        );
        ui.add_space(16.);
        if self.history.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(90.);
                icons::label(ui, Icon::History, p.secondary);
                ui.add_space(10.);
                ui.label(
                    RichText::new(self.t("还没有操作记录", "No activity yet"))
                        .size(18.)
                        .strong(),
                );
                ui.label(
                    RichText::new(self.t(
                        "复制或移动文件后，可以在这里查看与撤销。",
                        "Your copies and moves will appear here.",
                    ))
                    .color(p.secondary),
                );
            });
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for plan in self.history.clone() {
                    egui::Frame::new()
                        .fill(p.surface)
                        .corner_radius(8)
                        .inner_margin(14)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                icons::label(
                                    ui,
                                    if plan.state == PlanState::Completed {
                                        Icon::Check
                                    } else {
                                        Icon::History
                                    },
                                    p.accent,
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(state_name(plan.state, self.zh)).strong(),
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "{} · {} · {}",
                                            item_count(plan.items.len(), self.zh, true),
                                            size(plan.bytes()),
                                            &plan.id[..8]
                                        ))
                                        .small()
                                        .color(p.secondary),
                                    );
                                });
                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
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
                                    if !matches!(
                                        plan.state,
                                        PlanState::Prepared
                                            | PlanState::Completed
                                            | PlanState::RolledBack
                                    ) && ui
                                        .add_enabled(
                                            !self.busy,
                                            egui::Button::new(self.t("继续…", "Resume…")),
                                        )
                                        .clicked()
                                    {
                                        if plan.rollback_requested {
                                            self.confirm_undo = Some(plan.id.clone());
                                        } else {
                                            self.plan = Some(plan.clone());
                                        }
                                    }
                                    if ui.button(self.t("查看", "Review")).clicked() {
                                        self.plan = Some(plan.clone());
                                    }
                                });
                            });
                            if let Some(error) = plan.error {
                                ui.colored_label(ui.visuals().error_fg_color, error);
                            }
                        });
                    ui.add_space(4.);
                }
            });
    }
}

impl App {
    fn content(&mut self, ui: &mut egui::Ui) {
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
        let p = Palette::new(self.light);
        self.sidebar(ui);
        self.toolbar(ui);
        egui::Panel::bottom("status")
            .exact_size(28.)
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(16, 5)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.busy {
                        ui.add(egui::Spinner::new().size(12.));
                    } else {
                        ui.painter().circle_filled(
                            ui.cursor().min + egui::vec2(3., 7.),
                            2.5,
                            p.secondary,
                        );
                        ui.add_space(10.);
                    }
                    ui.add(
                        egui::Label::new(
                            RichText::new(if matches!(self.message.as_str(), "就绪" | "Ready") {
                                self.t("就绪", "Ready")
                            } else {
                                &self.message
                            })
                            .size(11.)
                            .color(p.secondary),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&self.message);
                    if self.progress.1 > 0 {
                        ui.add(
                            egui::ProgressBar::new(self.progress.0 as f32 / self.progress.1 as f32)
                                .desired_width(120.)
                                .show_percentage(),
                        );
                    }
                });
            });
        if self.plan.is_some() {
            egui::Panel::bottom("review")
                .resizable(true)
                .default_size(218.)
                .min_size(180.)
                .max_size(340.)
                .frame(
                    egui::Frame::new()
                        .fill(p.surface)
                        .inner_margin(egui::Margin::symmetric(20, 12))
                        .stroke(Stroke::new(1., p.line)),
                )
                .show(ui, |ui| {
                    self.plan_view(ui);
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.background))
            .show(ui, |ui| {
                if self.activity {
                    egui::Frame::new().inner_margin(22).show(ui, |ui| {
                        self.activity_view(ui);
                    });
                } else {
                    ui.spacing_mut().item_spacing.x = 0.;
                    ui.columns(2, |cols| {
                        self.pane(&mut cols[0], 0);
                        self.pane(&mut cols[1], 1);
                    });
                    let rect = ui.max_rect();
                    ui.painter().line_segment(
                        [rect.center_top(), rect.center_bottom()],
                        Stroke::new(1., p.line),
                    );
                }
            });
        if let Some(id) = self.confirm_undo.clone() {
            egui::Modal::new(egui::Id::unique("confirm_undo")).show(&ctx,|ui| {
                ui.set_width(380.);
                ui.label(RichText::new(self.t("撤销这次操作？", "Undo this operation?")).size(20.).strong());
                ui.add_space(10.);
                ui.label(self.t("只有内容与记录一致的文件才会被改动；原路径被占用时会停止。", "Only unchanged recorded files are modified. Recovery stops if an original path is occupied."));
                ui.add_space(16.);
                ui.with_layout(egui::Layout::right_to_left(Align::Center),|ui| {
                    if ui.add_enabled(!self.busy,egui::Button::new(self.t("确认撤销", "Confirm undo"))).clicked() {self.confirm_undo=None;self.send(Job::Undo(id));}
                    if ui.button(self.t("取消", "Cancel")).clicked() {self.confirm_undo=None;}
                });
            });
        }
        if let Some((path, text)) = &self.preview {
            let mut open = true;
            egui::Window::new(folder_name(path))
                .open(&mut open)
                .default_size([640., 420.])
                .show(&ctx, |ui| {
                    ui.label(
                        RichText::new(path.display().to_string())
                            .small()
                            .color(p.secondary),
                    );
                    ui.separator();
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

fn folder_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn item_count(count: usize, zh: bool, files: bool) -> String {
    if zh {
        format!("{count} {}", if files { "个文件" } else { "项" })
    } else {
        format!(
            "{count} {}{}",
            if files { "file" } else { "item" },
            if count == 1 { "" } else { "s" }
        )
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.content(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_workspace_fits_supported_window_sizes_in_both_themes() {
        let fixture = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(fixture.path()).unwrap();
        let mut engine = Engine::open(&root.join("state"), std::slice::from_ref(&root)).unwrap();
        std::fs::create_dir(root.join("Inbox")).unwrap();
        std::fs::create_dir(root.join("Library")).unwrap();
        let source = root.join("Inbox/sample.txt");
        std::fs::write(&source, "Generated layout fixture").unwrap();
        let plan = engine
            .prepare(vec![Request {
                action: Action::Copy,
                source: source.clone(),
                destination: root.join("Library/sample.txt"),
            }])
            .unwrap();
        let (tx, _jobs) = mpsc::channel();
        let (_events, rx) = mpsc::channel();
        let mut app = App {
            scope: engine.scope().clone(),
            panes: [
                Pane::new(root.join("Inbox")),
                Pane::new(root.join("Library")),
            ],
            active: 0,
            tx,
            rx,
            busy: false,
            progress: (0, 0),
            plan: Some(plan.clone()),
            history: vec![plan],
            activity: false,
            clipboard: None,
            message: "Ready".into(),
            zh: false,
            light: false,
            confirm_undo: None,
            preview: None,
            started: Instant::now(),
            smoke_seconds: None,
        };
        app.panes[0].entries.push(Entry {
            path: source.clone(),
            name: "sample.txt".into(),
            directory: false,
            link: false,
            bytes: 24,
            modified: None,
            error: None,
        });
        app.panes[0].visible.push(0);
        app.panes[0].selected.insert(source);
        for light in [false, true] {
            for zh in [false, true] {
                for size in [egui::vec2(960., 640.), egui::vec2(1240., 800.)] {
                    let ctx = egui::Context::default();
                    super::super::theme::install(&ctx);
                    ctx.set_theme(if light {
                        egui::Theme::Light
                    } else {
                        egui::Theme::Dark
                    });
                    app.light = light;
                    app.zh = zh;
                    for activity in [false, true] {
                        app.activity = activity;
                        for _ in 0..3 {
                            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
                            let mut output = ctx.run_ui(egui::RawInput {screen_rect: Some(viewport), ..Default::default()}, |ui| {
                                app.content(ui);
                                let used = ui.min_rect();
                                assert!(used.max.x <= size.x + 1. && used.max.y <= size.y + 1., "layout overflow: {used:?}, {size:?}, light={light}, zh={zh}, activity={activity}");
                            });
                            assert!(!output.shapes.is_empty());
                            output.textures_delta.clear();
                        }
                    }
                }
            }
        }
    }
}
