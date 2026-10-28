//! The Repositories pane: the repository list, in groups, with the open repository
//! expandable into its directory tree.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, pos2, vec2};
use snout_core::Tracking;
pub(super) use snout_core::groups::{RepoGroup, group_name_problem, move_to_group};
use snout_core::groups::{arrange, short};

use super::{Action, SnoutApp, dir_name, open_menu_items, pane_title, reveal_items, survey};
use crate::art::{self, Glyph, Paint as _};
use crate::theme::{Palette, mix};

/// What a repository row carries while it is dragged.
struct Dragged(PathBuf);

/// Where the list should file a dropped repository: in a group, or in none.
type Dropped = Option<(PathBuf, Option<String>)>;

const ROW_HEIGHT: f32 = 24.0;

impl SnoutApp {
    pub(super) fn repos_ui(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            pane_title(ui, "Repositories");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let add = art::icon_button(ui, Glyph::Plus, true).on_hover_text("Add a repository or a group");
                egui::Popup::menu(&add).show(|ui| {
                    open_menu_items(ui, actions);
                    ui.separator();
                    if ui.button("New group…").clicked() {
                        actions.push(Action::ShowNewGroup(None));
                    }
                });
            });
        });
        ui.separator();
        if self.settings.repos.len() > 6 {
            ui.add(egui::TextEdit::singleline(&mut self.repo_filter).hint_text("Filter").desired_width(f32::INFINITY));
            ui.add_space(4.0);
        }

        let (groups, loose) = arrange(&self.settings.repos, &self.settings.groups);
        let names: Vec<String> = groups.iter().map(|g| g.name.clone()).collect();
        let needle = self.repo_filter.trim().to_lowercase();
        let mut dropped: Dropped = None;

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            if self.settings.repos.is_empty() {
                ui.weak("No repositories yet.\nUse +, ⌘O, or drop a folder here.");
            }
            if !needle.is_empty() {
                // Matches in one flat list, each still filed under its group.
                let hits = groups
                    .iter()
                    .flat_map(|g| g.repos.iter().map(move |p| (p, Some(g.name.as_str()))))
                    .chain(loose.iter().map(|p| (p, None)))
                    .filter(|(p, g)| {
                        dir_name(p).to_lowercase().contains(&needle)
                            || g.is_some_and(|g| g.to_lowercase().contains(&needle))
                    });
                let mut any = false;
                for (path, group) in hits {
                    any = true;
                    self.repo_entry(ui, path, group, 0, &names, actions, &mut dropped);
                }
                if !any {
                    ui.weak("No matches");
                }
                return;
            }
            for g in &groups {
                self.group_entry(ui, g, &names, actions, &mut dropped);
            }
            if !groups.is_empty() && !loose.is_empty() {
                ui.add_space(6.0);
            }
            for path in &loose {
                self.repo_entry(ui, path, None, 0, &names, actions, &mut dropped);
            }
            // The space below the list takes a repository out of its group.
            let rest =
                ui.allocate_response(vec2(ui.available_width(), ui.available_height().max(ROW_HEIGHT)), Sense::hover());
            if let Some(d) = rest.dnd_hover_payload::<Dragged>()
                && groups.iter().any(|g| g.repos.contains(&d.0))
            {
                let p = Palette::of(ui.visuals());
                let zone = Rect::from_min_size(rest.rect.min, vec2(rest.rect.width(), ROW_HEIGHT + 8.0));
                ui.painter().rect(zone, 6.0, p.wash(p.ok), Stroke::new(1.0, p.ok), egui::StrokeKind::Inside);
                ui.painter().text(
                    zone.center(),
                    egui::Align2::CENTER_CENTER,
                    "Drop here to take it out of its group",
                    FontId::proportional(12.0),
                    p.fg_dim,
                );
            }
            if let Some(d) = rest.dnd_release_payload::<Dragged>() {
                dropped = Some((d.0.clone(), None));
            }
        });
        if let Some((path, group)) = dropped {
            actions.push(Action::MoveToGroup(path, group));
        }
    }

    fn group_entry(
        &mut self,
        ui: &mut egui::Ui,
        g: &RepoGroup,
        names: &[String],
        actions: &mut Vec<Action>,
        dropped: &mut Dropped,
    ) {
        let p = Palette::of(ui.visuals());
        let id = egui::Id::unique(("repo-group", &g.name));
        let open = is_open(ui, id, true);
        let mut chips = vec![(g.repos.len().to_string(), p.fg_dim)];
        let mut hover = "Drag repositories here to add them".to_owned();
        // Closed, the group still says when something in it has commits to pull.
        let behind: u32 =
            g.repos.iter().filter_map(|r| self.tracking.get(r).copied().flatten()).map(|t| t.behind).sum();
        if let Some((chip, line)) = survey::describe(Tracking { ahead: 0, behind }).filter(|_| !open) {
            chips.insert(0, (chip, p.accent));
            hover = format!("{line}\n{hover}");
        }
        let kind = RowKind { open: Some(open), ..REPO };
        let (resp, _) = list_row(ui, Glyph::Folder, &g.name, kind, &chips);
        let resp = resp.on_hover_text(hover);
        if resp.clicked() {
            set_open(ui, id, !open);
        }
        if let Some(d) = resp.dnd_release_payload::<Dragged>() {
            *dropped = Some((d.0.clone(), Some(g.name.clone())));
        }
        resp.context_menu(|ui| {
            if ui.button("Rename…").clicked() {
                actions.push(Action::ShowRenameGroup(g.name.clone()));
            }
            if ui.button("Delete group").on_hover_text("Its repositories stay in the list").clicked() {
                actions.push(Action::DeleteGroup(g.name.clone()));
            }
            ui.separator();
            if ui.button("New group…").clicked() {
                actions.push(Action::ShowNewGroup(None));
            }
        });
        if !open {
            return;
        }
        if g.repos.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(INDENT + ARROW + 2.0);
                ui.weak("Drag repositories here");
            });
        }
        for path in &g.repos {
            self.repo_entry(ui, path, Some(&g.name), 1, names, actions, dropped);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repo_entry(
        &mut self,
        ui: &mut egui::Ui,
        path: &Path,
        group: Option<&str>,
        depth: u8,
        names: &[String],
        actions: &mut Vec<Action>,
        dropped: &mut Dropped,
    ) {
        if self.settings.current.as_deref() == Some(path) {
            return self.repo_tree(ui, path, group, depth, names, actions, dropped);
        }
        let p = Palette::of(ui.visuals());
        let name = dir_name(path);
        let missing = !path.is_dir();
        let (glyph, mut chips) =
            if missing { (Glyph::Alert, vec![("missing".to_owned(), p.error)]) } else { (Glyph::Repo, Vec::new()) };
        let mut hover = path.display().to_string();
        if let Some((chip, line)) = self.tracking.get(path).copied().flatten().and_then(survey::describe) {
            chips.push((chip, p.accent));
            hover = format!("{hover}\n{line}");
        }
        let kind = RowKind { dim: missing, depth, ..REPO };
        let (resp, _) = list_row(ui, glyph, &name, kind, &chips);
        let resp = resp.on_hover_text(hover);
        drag_source(ui, &resp, path, &name);
        if resp.clicked() {
            actions.push(Action::OpenRepo(path.to_owned()));
        }
        if let Some(d) = resp.dnd_release_payload::<Dragged>() {
            *dropped = Some((d.0.clone(), group.map(str::to_owned)));
        }
        resp.context_menu(|ui| repo_menu(ui, path, group, names, true, actions));
    }

    /// The open repository, expandable into its directory tree. Picking a directory
    /// narrows the Files pane to it; picking the repository shows everything again.
    #[allow(clippy::too_many_arguments)]
    fn repo_tree(
        &mut self,
        ui: &mut egui::Ui,
        root: &Path,
        group: Option<&str>,
        depth: u8,
        names: &[String],
        actions: &mut Vec<Action>,
        dropped: &mut Dropped,
    ) {
        let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for dir in &self.dirs {
            let parent = dir.rfind('/').map_or("", |i| &dir[..i]);
            children.entry(parent).or_default().push(dir);
        }
        let mut changed: HashMap<&str, usize> = HashMap::new();
        for f in &self.status.files {
            let mut end = f.path.len();
            while let Some(slash) = f.path[..end].rfind('/') {
                *changed.entry(&f.path[..slash]).or_default() += 1;
                end = slash;
            }
        }
        let tree = DirTree { root, children: &children, changed: &changed, selected: self.dir_filter.as_deref() };
        let mut picked: Option<Option<String>> = None;

        let p = Palette::of(ui.visuals());
        let b = &self.status.branch;
        let mut head = short(&self.head_label(), 20);
        if b.ahead > 0 {
            head += &format!(" ↑{}", b.ahead);
        }
        if b.behind > 0 {
            head += &format!(" ↓{}", b.behind);
        }
        let mut chips = vec![(head, if b.head.is_some() { p.ok } else { p.warn })];
        if !self.status.files.is_empty() {
            chips.push((self.status.files.len().to_string(), p.accent));
        }
        let name = dir_name(root);
        let id = egui::Id::unique(("repo-tree", root));
        let open = is_open(ui, id, true);
        let has_dirs = children.contains_key("");
        let kind = RowKind { selected: self.dir_filter.is_none(), depth, open: has_dirs.then_some(open), ..REPO };
        let (resp, toggled) = list_row(ui, Glyph::Repo, &name, kind, &chips);
        let resp = resp.on_hover_text(format!("{}\n{} changed files", root.display(), self.status.files.len()));
        drag_source(ui, &resp, root, &name);
        if toggled {
            set_open(ui, id, !open);
        } else if resp.clicked() {
            picked = Some(None);
        }
        if let Some(d) = resp.dnd_release_payload::<Dragged>() {
            *dropped = Some((d.0.clone(), group.map(str::to_owned)));
        }
        resp.context_menu(|ui| repo_menu(ui, root, group, names, false, actions));
        if open {
            tree.show(ui, "", depth + 1, &mut picked, actions);
        }
        if let Some(dir) = picked {
            self.dir_filter = dir;
            self.anchor = None;
        }
    }
}

fn repo_menu(
    ui: &mut egui::Ui,
    path: &Path,
    group: Option<&str>,
    names: &[String],
    openable: bool,
    actions: &mut Vec<Action>,
) {
    if openable && ui.button("Open").clicked() {
        actions.push(Action::OpenRepo(path.to_owned()));
    }
    reveal_items(ui, path, actions);
    ui.separator();
    ui.menu_button("Move to group", |ui| {
        for name in names.iter().filter(|n| Some(n.as_str()) != group) {
            if ui.button(name).clicked() {
                actions.push(Action::MoveToGroup(path.to_owned(), Some(name.clone())));
            }
        }
        if group.is_some() && ui.button("No group").clicked() {
            actions.push(Action::MoveToGroup(path.to_owned(), None));
        }
        if names.iter().any(|n| Some(n.as_str()) != group) || group.is_some() {
            ui.separator();
        }
        if ui.button("New group…").clicked() {
            actions.push(Action::ShowNewGroup(Some(path.to_owned())));
        }
    });
    ui.separator();
    if ui.button("Remove from list").clicked() {
        actions.push(Action::RemoveRepo(path.to_owned()));
    }
}

fn is_open(ui: &egui::Ui, id: egui::Id, default: bool) -> bool {
    ui.ctx().data_mut(|d| *d.get_persisted_mut_or(id, default))
}

fn set_open(ui: &egui::Ui, id: egui::Id, open: bool) {
    ui.ctx().data_mut(|d| d.insert_persisted(id, open));
}

/// Width of the arrow column at the start of every row, and the indent of each level.
const ARROW: f32 = 14.0;
const INDENT: f32 = 14.0;

/// How a [`list_row`] looks and behaves.
#[derive(Clone, Copy, Default)]
struct RowKind {
    selected: bool,
    dim: bool,
    /// Lights up under a dragged repository: repository and group rows file it where they are.
    drop: bool,
    /// Nesting level: groups and loose repositories are 0.
    depth: u8,
    /// `Some(open)` draws an arrow: › closed, ⌄ open.
    open: Option<bool>,
}

const REPO: RowKind = RowKind { selected: false, dim: false, drop: true, depth: 0, open: None };

/// A row of the tree: arrow column, icon, name (cut to fit), and small chips at the right
/// end. Every row keeps the arrow column, so icons at one level line up whether or not
/// the row opens. Also returns whether the click landed on the arrow.
fn list_row(
    ui: &mut egui::Ui,
    glyph: Glyph,
    name: &str,
    kind: RowKind,
    chips: &[(String, Color32)],
) -> (egui::Response, bool) {
    let p = Palette::of(ui.visuals());
    let sense = if kind.drop { Sense::click_and_drag() } else { Sense::click() };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), sense);
    let (selected, dim) = (kind.selected, kind.dim);
    let drop_target = kind.drop && resp.dnd_hover_payload::<Dragged>().is_some();
    let left = rect.left() + 2.0 + f32::from(kind.depth) * INDENT;
    let arrow = Rect::from_min_size(pos2(left, rect.top()), vec2(ARROW, rect.height()));
    let toggled = kind.open.is_some()
        && resp.clicked()
        && resp.interact_pointer_pos().is_some_and(|pos| arrow.expand2(vec2(3.0, 0.0)).contains(pos));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if drop_target {
            painter.rect(rect, 6.0, p.wash(p.ok), Stroke::new(1.0, p.ok), egui::StrokeKind::Inside);
        } else if selected {
            painter.rect_filled(rect, 6.0, ui.visuals().selection.bg_fill);
        } else if resp.hovered() {
            painter.rect_filled(rect, 6.0, p.button_hover);
        }
        let ink = if dim { p.fg_dim } else { p.fg };
        if let Some(open) = kind.open {
            let c = arrow.center();
            let points = if open {
                vec![c + vec2(-3.5, -1.75), c + vec2(0.0, 1.75), c + vec2(3.5, -1.75)]
            } else {
                vec![c + vec2(-1.75, -3.5), c + vec2(1.75, 0.0), c + vec2(-1.75, 3.5)]
            };
            painter.add(egui::Shape::line(points, Stroke::new(1.6, p.fg_dim)));
        }
        let icon = Rect::from_min_size(pos2(arrow.right() + 1.0, rect.center().y - 8.0), vec2(16.0, 16.0));
        glyph.drawing().paint(painter, icon, Some(ink));

        let mut right = rect.right() - 3.0;
        for (text, color) in chips.iter().rev() {
            let galley = painter.layout_no_wrap(text.clone(), FontId::proportional(11.5), *color);
            let size = vec2(galley.size().x + 12.0, 17.0);
            let chip = Rect::from_min_size(pos2(right - size.x, rect.center().y - size.y / 2.0), size);
            if chip.left() < icon.right() + 40.0 {
                break;
            }
            painter.rect(
                chip,
                size.y / 2.0,
                p.wash(*color),
                Stroke::new(1.0, mix(*color, 0.4, p.panel)),
                egui::StrokeKind::Inside,
            );
            painter.galley(chip.center() - galley.size() / 2.0, galley, *color);
            right = chip.left() - 4.0;
        }

        let font = egui::TextStyle::Body.resolve(ui.style());
        let mut job = egui::text::LayoutJob::simple_singleline(name.to_owned(), font, ink);
        job.wrap = egui::text::TextWrapping::truncate_at_width((right - icon.right() - 10.0).max(10.0));
        let galley = painter.layout_job(job);
        painter.galley(pos2(icon.right() + 7.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, name));
    (resp, toggled)
}

/// The directory tree of the open repository, for [`SnoutApp::repo_tree`].
struct DirTree<'a> {
    root: &'a Path,
    children: &'a BTreeMap<&'a str, Vec<&'a str>>,
    /// Changed files under each directory, recursively.
    changed: &'a HashMap<&'a str, usize>,
    selected: Option<&'a str>,
}

impl DirTree<'_> {
    fn show(
        &self,
        ui: &mut egui::Ui,
        parent: &str,
        depth: u8,
        picked: &mut Option<Option<String>>,
        actions: &mut Vec<Action>,
    ) {
        let Some(dirs) = self.children.get(parent) else { return };
        let p = Palette::of(ui.visuals());
        for &dir in dirs {
            let name = dir.rsplit('/').next().unwrap_or(dir);
            let changed = self.changed.get(dir).copied().unwrap_or(0);
            let id = egui::Id::unique(("dir", self.root, dir));
            let open = self.children.contains_key(dir).then(|| is_open(ui, id, false));
            let chips: Vec<(String, Color32)> =
                if changed > 0 { vec![(changed.to_string(), p.accent)] } else { Vec::new() };
            let kind = RowKind { selected: self.selected == Some(dir), depth, open, ..RowKind::default() };
            let (mut resp, toggled) = list_row(ui, Glyph::Folder, name, kind, &chips);
            if changed > 0 {
                resp = resp.on_hover_text(format!("{changed} changed"));
            }
            if toggled {
                set_open(ui, id, open != Some(true));
            } else if resp.clicked() {
                *picked = Some(Some(dir.to_owned()));
            }
            resp.context_menu(|ui| reveal_items(ui, &self.root.join(dir), actions));
            if open == Some(true) {
                self.show(ui, dir, depth + 1, picked, actions);
            }
        }
    }
}

/// Make a row draggable onto groups, with its name following the pointer.
fn drag_source(ui: &egui::Ui, resp: &egui::Response, path: &Path, name: &str) {
    resp.dnd_set_drag_payload(Dragged(path.to_owned()));
    if !resp.dragged() {
        return;
    }
    let Some(pos) = ui.ctx().pointer_interact_pos() else { return };
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    let p = Palette::of(ui.visuals());
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, resp.id.with("dragged")));
    let galley = painter.layout_no_wrap(name.to_owned(), FontId::proportional(13.0), p.fg);
    let size = vec2(galley.size().x + 38.0, 26.0);
    let rect = Rect::from_min_size(pos + vec2(10.0, 8.0), size);
    painter.rect(rect, 8.0, p.card, Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    let icon = Rect::from_min_size(pos2(rect.left() + 8.0, rect.center().y - 8.0), vec2(16.0, 16.0));
    Glyph::Repo.drawing().paint(&painter, icon, Some(p.fg));
    painter.galley(pos2(icon.right() + 6.0, rect.center().y - galley.size().y / 2.0), galley, p.fg);
}
