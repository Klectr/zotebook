//! Folders: new notes inside a folder, new folders, moving notes by drag, and the tree's menu.

use rune::{Drag, Phase, TextEdit, Tx};

use crate::app::{App, Sel};
use crate::md;
use crate::vault::Op;

/// A note being dragged in the file tree.
#[derive(Clone, Debug)]
pub struct DragNote {
    pub path: String,
    /// The pointer, in window points.
    pub x: f32,
    pub y: f32,
    /// Past the threshold: a drag, not a click.
    pub active: bool,
    /// The folder under the pointer (`""` is the root), if the note can go there.
    pub target: Option<String>,
    /// The pointer is over the Pinned group, and the note is not pinned yet.
    pub pin: bool,
}

/// What the tree's menu was opened on.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuOn {
    Folder(String),
    Note(String),
    Root,
}

#[derive(Clone, Debug)]
pub struct TreeMenu {
    pub x: f32,
    pub y: f32,
    pub on: MenuOn,
    /// The menu asks before it deletes the folder it was opened on.
    pub confirm: bool,
}

/// A folder being named in the tree.
pub struct NewFolder {
    pub parent: String,
    pub edit: TextEdit,
}

/// The picker that gives a folder, or a note, an emoji icon.
pub struct FolderIconPicker {
    pub folder: String,
    /// The note whose icon this picks, instead of the folder's. The icon is the first
    /// character of the note's file name, as a folder's is of the folder name.
    pub note: Option<String>,
    /// Filters the grid of common emoji.
    pub search: TextEdit,
    /// Any other emoji, typed or pasted.
    pub custom: TextEdit,
    pub error: Option<String>,
}

/// A tree row key holds its path with `|` for `/`, since `/` separates keys in element paths.
pub fn row_key(kind: &str, path: &str) -> String {
    format!("{kind}:{}", path.replace('/', "|"))
}

/// The folder an element path points into: a folder row is that folder, a note row is its
/// note's folder, and the tree's empty space or the folder name is the root.
pub fn folder_at(path: &str) -> Option<String> {
    for seg in path.split('/').rev() {
        if let Some(p) = seg.strip_prefix("dir:") {
            return Some(p.replace('|', "/"));
        }
        if let Some(p) = seg.strip_prefix("file:") {
            return Some(md::dir(&p.replace('|', "/")).to_string());
        }
    }
    path.split('/').any(|s| matches!(s, "sidebar" | "side-scroll" | "vault-head")).then(String::new)
}

/// Splits `Drafts/Ideas/Name` into a clean folder path and a clean note name.
fn split_path(name: &str) -> (String, String) {
    let parts: Vec<String> = name.split('/').map(md::clean_name).filter(|p| !p.is_empty() && p != "." && p != "..").collect();
    match parts.split_last() {
        Some((last, dirs)) => (dirs.join("/"), last.clone()),
        None => (String::new(), String::new()),
    }
}

impl App {
    /// A new note in `folder` (`None`: the folder from settings). A `name` with slashes, such as
    /// `Drafts/Idea`, makes the note in that folder, creating any folders it names.
    pub fn new_note_in(&mut self, folder: Option<&str>, name: Option<&str>, tx: &mut Tx) {
        if self.vault.is_none() {
            return;
        }
        let mut base_folder = match folder {
            Some(f) => f.to_string(),
            None => {
                let f = self.settings().new_folder.clone();
                if !f.is_empty() && !self.folder_list().contains(&f) { String::new() } else { f }
            }
        };
        let (sub, bn) = match name {
            Some(n) if n.contains('/') => {
                let (d, b) = split_path(n);
                // A path from the root wins over the settings folder.
                base_folder = String::new();
                (d, b)
            }
            Some(n) => (String::new(), md::clean_name(n)),
            None => (String::new(), String::new()),
        };
        let folder = if sub.is_empty() { base_folder } else { md::join(&base_folder, &sub) };
        let bn = if bn.is_empty() { "Untitled".to_string() } else { bn };
        let p = self.unique_path(&folder, &bn);
        let content = if name.is_some() { format!("# {bn}\n\n") } else { String::new() };
        if let Some(v) = &mut self.vault {
            v.files.insert(p.clone(), content.clone());
            v.born.insert(p.clone(), crate::app::now_ns());
            let mut d = folder.clone();
            while !d.is_empty() {
                if !v.folders.contains(&d) {
                    v.folders.push(d.clone());
                }
                d = md::dir(&d).to_string();
            }
            v.folders.sort();
        }
        self.rebuild_names_pub();
        self.mark_dirty_pub(&p);
        self.screen = crate::app::Screen::Vault;
        self.settings_open = false;
        self.tree_menu = None;
        let end = content.len();
        self.open_file(&p, Some(Sel { start: end, end, line: None, force_source: false }), tx);
        if name.is_none() {
            self.mode = crate::app::Mode::Source;
            self.focus = false;
            self.start_rename(tx);
        }
    }

    /// The note belonging to `folder`: a note inside it with the folder's own name.
    pub fn folder_note(&self, folder: &str) -> Option<String> {
        if folder.is_empty() {
            return None;
        }
        self.files().keys().find(|p| md::is_folder_note_in(p, folder)).cloned()
    }

    /// Opens `folder`'s note, making it when the folder has none. A folder the tree has no row
    /// for still toggles, as before.
    pub fn open_folder(&mut self, folder: &str, tx: &mut Tx) {
        if folder.is_empty() || !self.folder_list().iter().any(|f| f == folder) {
            self.toggle_folder(folder);
            return;
        }
        // An existing note opens in the mode the app is in. A new one is empty, so it opens in
        // the editor, as any new note does.
        let (note, made) = match self.folder_note(folder) {
            Some(p) => (Some(p), false),
            None => (self.create_folder_note(folder), true),
        };
        if let Some(p) = note {
            let end = self.files().get(&p).map_or(0, String::len);
            self.open_file(&p, Some(Sel { start: end, end, line: None, force_source: made }), tx);
        }
    }

    /// Makes `<folder>/<folder name>.md` when it is not there, and returns its path.
    fn create_folder_note(&mut self, folder: &str) -> Option<String> {
        if folder.is_empty() {
            return None;
        }
        if let Some(p) = self.folder_note(folder) {
            return Some(p);
        }
        let name = md::folder_name(folder);
        let name = if name.is_empty() { md::folder_segment(folder) } else { name };
        let path = md::folder_note_path(folder);
        let text = format!("# {name}\n\n");
        let v = self.vault.as_mut()?;
        v.files.insert(path.clone(), text);
        v.born.insert(path.clone(), crate::app::now_ns());
        let mut d = folder.to_string();
        while !d.is_empty() {
            if !v.folders.contains(&d) {
                v.folders.push(d.clone());
            }
            d = md::dir(&d).to_string();
        }
        v.folders.sort();
        self.rebuild_names_pub();
        self.mark_dirty_pub(&path);
        self.persist_later();
        Some(path)
    }

    /// Opens the note of the active note's folder, for the command palette.
    pub fn open_active_folder_note(&mut self, tx: &mut Tx) {
        let Some(active) = self.active.clone() else { return };
        let folder = md::dir(&active).to_string();
        if !folder.is_empty() {
            self.open_folder(&folder, tx);
        }
    }

    /// Starts naming a new folder inside `parent` (`""` is the root).
    pub fn start_new_folder(&mut self, parent: &str, tx: &mut Tx) {
        let mut edit = TextEdit::new("");
        edit.focused = true;
        if !parent.is_empty() {
            self.expanded.insert(parent.to_string());
        }
        self.persist.left_open = true;
        self.left_panel = crate::app::Panel::Files;
        self.new_folder = Some(NewFolder { parent: parent.to_string(), edit });
        self.tree_menu = None;
        tx.focus("new-folder", false);
    }

    pub fn commit_new_folder(&mut self) {
        let Some(nf) = self.new_folder.take() else { return };
        let name = md::clean_name(&nf.edit.text);
        if name.is_empty() || name.starts_with('.') {
            return;
        }
        let path = md::join(&nf.parent, &name);
        let Some(v) = &mut self.vault else { return };
        if v.folders.contains(&path) {
            self.notice = Some(format!("A folder named {name} is already there."));
            return;
        }
        v.folders.push(path.clone());
        v.folders.sort();
        let root = v.path.clone();
        self.writer.send(Op::Mkdir { root, rel: path.clone() });
        self.expanded.insert(path);
        if !nf.parent.is_empty() {
            self.expanded.insert(nf.parent);
        }
        self.persist_later();
        self.sync_after_save();
    }

    /// Opens the icon picker for `folder`.
    pub fn start_folder_icon(&mut self, folder: &str, tx: &mut Tx) {
        if folder.is_empty() || !self.folder_list().iter().any(|f| f == folder) {
            return;
        }
        let mut search = TextEdit::new("");
        search.focused = true;
        self.folder_icon = Some(FolderIconPicker { folder: folder.to_string(), note: None, search, custom: TextEdit::new(""), error: None });
        self.tree_menu = None;
        self.vault_menu = false;
        tx.focus("folder-icon-search", false);
    }

    /// Opens the icon picker for the note at `path`. A folder note that shows its folder's
    /// icon changes the folder's icon instead, since that is the icon it shows.
    pub fn start_note_icon(&mut self, path: &str, tx: &mut Tx) {
        if !self.files().contains_key(path) {
            return;
        }
        if md::name_icon(md::base(path)).is_none() && md::note_icon(path).is_some() {
            self.start_folder_icon(md::dir(path), tx);
            return;
        }
        let mut search = TextEdit::new("");
        search.focused = true;
        let folder = md::dir(path).to_string();
        self.folder_icon = Some(FolderIconPicker { folder, note: Some(path.to_string()), search, custom: TextEdit::new(""), error: None });
        self.tree_menu = None;
        self.vault_menu = false;
        self.renaming = None;
        tx.focus("folder-icon-search", false);
    }

    pub fn close_folder_icon(&mut self, tx: &mut Tx) {
        self.folder_icon = None;
        self.focus_editor(tx);
    }

    /// Gives the picker's folder `icon`, or removes its icon with `None`. The icon is the first
    /// character of the folder name on disk, so this renames the folder.
    pub fn apply_folder_icon(&mut self, icon: Option<&str>, tx: &mut Tx) {
        let Some(picker) = &self.folder_icon else { return };
        if let Some(note) = picker.note.clone() {
            self.apply_note_icon(&note, icon, tx);
            return;
        }
        let folder = picker.folder.clone();
        let segment = md::folder_segment(&folder);
        let name = md::name_without_icon(segment);
        if name.is_empty() {
            if let Some(p) = &mut self.folder_icon {
                p.error = Some("The folder needs a name besides its icon.".into());
            }
            return;
        }
        let old = md::name_icon(segment);
        if old == icon {
            self.close_folder_icon(tx);
            return;
        }
        let name = name.to_string();
        let new_segment = match icon {
            Some(i) => format!("{i} {name}"),
            None => name,
        };
        let to = md::join(md::dir(&folder), &new_segment);
        if !self.rename_folder(&folder, &to) {
            return;
        }
        self.folder_icon = None;
        self.focus_editor(tx);
    }

    /// Gives the note at `path` the picker's `icon`, or removes it with `None`, by renaming the
    /// note. Links by the name without an icon keep working, so only links that spell out the
    /// old icon change.
    fn apply_note_icon(&mut self, path: &str, icon: Option<&str>, tx: &mut Tx) {
        let base = md::base(path);
        let name = md::name_without_icon(base);
        if name.is_empty() {
            if let Some(p) = &mut self.folder_icon {
                p.error = Some("The note needs a name besides its icon.".into());
            }
            return;
        }
        if md::name_icon(base) == icon {
            self.close_folder_icon(tx);
            return;
        }
        let new_base = match icon {
            Some(i) => format!("{i} {name}"),
            None => name.to_string(),
        };
        let to = md::join(md::dir(path), &format!("{new_base}.md"));
        if self.files().contains_key(&to) {
            if let Some(p) = &mut self.folder_icon {
                p.error = Some(format!("A note named “{new_base}” is already here."));
            }
            return;
        }
        self.folder_icon = None;
        let update = base != name && self.settings().update_links;
        self.rename_note(path, &new_base, update);
        self.focus_editor(tx);
    }

    /// Applies the emoji in the picker's custom field.
    pub fn apply_custom_folder_icon(&mut self, tx: &mut Tx) {
        let Some(picker) = &self.folder_icon else { return };
        let Some(icon) = crate::emoji::single(&picker.custom.text) else {
            if let Some(p) = &mut self.folder_icon {
                p.error = Some("Type or paste one emoji.".into());
            }
            return;
        };
        let icon = icon.to_string();
        self.apply_folder_icon(Some(&icon), tx);
    }

    /// Applies the first emoji that matches the picker's search, for Enter in the search field.
    pub fn apply_searched_folder_icon(&mut self, tx: &mut Tx) {
        let Some(picker) = &self.folder_icon else { return };
        if picker.search.text.trim().is_empty() {
            return;
        }
        let Some(icon) = crate::emoji::search(&picker.search.text).first().copied() else { return };
        self.apply_folder_icon(Some(icon), tx);
    }

    /// Renames a folder, keeping its notes, tabs and links. The picker only ever changes the last
    /// part of the path, so the folder stays where it is.
    fn rename_folder(&mut self, from: &str, to: &str) -> bool {
        if from.is_empty() || from == to {
            return false;
        }
        if self.folder_list().iter().any(|f| f == to) || self.vault.as_ref().is_some_and(|v| v.path.join(to).exists()) {
            self.notice = Some(format!("A folder named {} is already there.", md::folder_name(to)));
            return false;
        }
        self.flush_notes();
        let Some(root) = self.vault.as_ref().map(|v| v.path.clone()) else { return false };
        let old_pre = format!("{from}/");
        let new_pre = format!("{to}/");
        let remap = |p: &str| -> String {
            if p == from {
                to.to_string()
            } else if let Some(rest) = p.strip_prefix(&old_pre) {
                format!("{new_pre}{rest}")
            } else {
                p.to_string()
            }
        };
        if let Some(v) = &mut self.vault {
            v.folders = std::mem::take(&mut v.folders).into_iter().map(|f| remap(&f)).collect();
            v.folders.sort();
            v.folders.dedup();
            v.files = std::mem::take(&mut v.files).into_iter().map(|(p, t)| (remap(&p), t)).collect();
            v.born = std::mem::take(&mut v.born).into_iter().map(|(p, b)| (remap(&p), b)).collect();
        }
        self.writer.send(Op::RenameFolder { root, from: from.to_string(), to: to.to_string() });
        // Links that spell out the old path follow it. Bare links reach the folder by its name.
        if self.settings().update_links {
            let mut rewritten = Vec::new();
            if let Some(v) = &mut self.vault {
                for (p, t) in v.files.iter_mut() {
                    let nt = md::rewrite_folder_links(t, from, to);
                    if nt != *t {
                        *t = nt;
                        rewritten.push(p.clone());
                    }
                }
            }
            for p in rewritten {
                let t = self.files().get(&p).cloned().unwrap_or_default();
                self.set_text(&p, t);
            }
        }
        self.tabs = std::mem::take(&mut self.tabs).into_iter().map(|p| remap(&p)).collect();
        self.recent = std::mem::take(&mut self.recent).into_iter().map(|p| remap(&p)).collect();
        self.pinned = std::mem::take(&mut self.pinned).into_iter().map(|p| remap(&p)).collect();
        self.expanded = std::mem::take(&mut self.expanded).into_iter().map(|p| remap(&p)).collect();
        self.unlocked = std::mem::take(&mut self.unlocked).into_iter().map(|(p, u)| (remap(&p), u)).collect();
        self.edits = std::mem::take(&mut self.edits).into_iter().map(|(p, e)| (remap(&p), e)).collect();
        self.active = self.active.as_deref().map(remap);
        self.persist.settings.new_folder = remap(&self.persist.settings.new_folder);
        if let Some(nf) = &mut self.new_folder {
            nf.parent = remap(&nf.parent);
        }
        self.dragging = None;
        self.rebuild_names_pub();
        self.persist_later();
        self.sync_after_save();
        true
    }

    /// Moves a note into `folder` (`""` is the root), keeping its tab, caret and history.
    pub fn move_note(&mut self, from: &str, folder: &str, tx: &mut Tx) {
        if md::dir(from) == folder || !self.files().contains_key(from) {
            return;
        }
        let name = from.rsplit('/').next().unwrap_or(from).to_string();
        let to = md::join(folder, &name);
        let exists = self.files().contains_key(&to) || self.vault.as_ref().is_some_and(|v| v.path.join(&to).exists());
        if exists {
            self.notice = Some(format!("{} already has a note named {}.", if folder.is_empty() { "The folder root" } else { folder }, md::base(&name)));
            return;
        }
        self.flush_notes();
        let update = self.settings().update_links;
        let Some(v) = &mut self.vault else { return };
        let root = v.path.clone();
        let text = v.files.remove(from).unwrap_or_default();
        v.files.insert(to.clone(), text);
        if let Some(b) = v.born.remove(from) {
            v.born.insert(to.clone(), b);
        }
        self.writer.send(Op::Rename { root, from: from.to_string(), to: to.clone() });
        // Links by note name still find it. Links that spell out the old folder follow it.
        let mut rewritten = Vec::new();
        if update {
            let (old_full, new_full) = (md::strip_md(from).to_string(), md::strip_md(&to).to_string());
            if let Some(v) = &mut self.vault {
                for (p, t) in v.files.iter_mut() {
                    let nt = md::rewrite_links(t, &old_full, &new_full);
                    if nt != *t {
                        *t = nt;
                        rewritten.push(p.clone());
                    }
                }
            }
        }
        if let Some(u) = self.unlocked.remove(from) {
            self.unlocked.insert(to.clone(), u);
        }
        if let Some(e) = self.edits.remove(from) {
            self.edits.insert(to.clone(), e);
        }
        for p in rewritten {
            let t = self.files().get(&p).cloned().unwrap_or_default();
            self.set_text(&p, t);
        }
        for t in self.tabs.iter_mut().chain(self.recent.iter_mut()).chain(self.pinned.iter_mut()) {
            if t == from {
                *t = to.clone();
            }
        }
        if self.active.as_deref() == Some(from) {
            self.active = Some(to.clone());
        }
        if !folder.is_empty() {
            self.expanded.insert(folder.to_string());
        }
        self.rebuild_names_pub();
        self.persist_later();
        self.sync_after_save();
        let _ = tx;
    }

    /// A file row's drag: past 6 points it becomes a move, and the folder under the pointer
    /// takes the note when the button comes up.
    pub fn drag_note(&mut self, path: &str, d: &Drag, tx: &mut Tx) {
        if d.cancelled() {
            self.dragging = None;
            return;
        }
        match d.phase {
            Phase::Down => {
                self.dragging = Some(DragNote { path: path.to_string(), x: d.window_point()[0], y: d.window_point()[1], active: false, target: None, pin: false });
            }
            Phase::Move => {
                let Some(dn) = &mut self.dragging else { return };
                dn.x = d.window_point()[0];
                dn.y = d.window_point()[1];
                if !dn.active && d.dx.abs() + d.dy.abs() > 6.0 {
                    dn.active = true;
                }
                if dn.active {
                    let (x, y) = (dn.x, dn.y);
                    tx.element_at(x, y, |s: &mut App, picked, _| {
                        let pin = picked.as_ref().is_some_and(|p| crate::pins::in_group(p.path()));
                        let target = if pin { None } else { picked.and_then(|p| folder_at(p.path())) };
                        let pinned = s.dragging.as_ref().is_some_and(|dn| s.is_pinned(&dn.path));
                        if let Some(dn) = &mut s.dragging {
                            dn.pin = pin && !pinned;
                            dn.target = target.filter(|f| *f != md::dir(&dn.path));
                        }
                    });
                }
            }
            Phase::Up => {
                if let Some(dn) = self.dragging.take() {
                    if dn.active {
                        if dn.pin {
                            self.pin(&dn.path);
                        } else if let Some(t) = dn.target {
                            self.move_note(&dn.path, &t, tx);
                        }
                    }
                }
            }
        }
    }

    pub fn open_tree_menu(&mut self, on: MenuOn, x: f32, y: f32) {
        self.tree_menu = Some(TreeMenu { x, y, on, confirm: false });
        self.vault_menu = false;
    }

    /// The notes inside `folder`, its subfolders included.
    pub fn notes_in(&self, folder: &str) -> Vec<String> {
        let pre = format!("{folder}/");
        self.files().keys().filter(|p| p.starts_with(&pre)).cloned().collect()
    }

    /// Deletes a folder from the menu: an empty folder goes at once, and a folder with notes
    /// asks first.
    pub fn ask_delete_folder(&mut self, folder: &str, x: f32, y: f32) {
        if self.notes_in(folder).is_empty() {
            self.tree_menu = None;
            self.delete_folder(folder);
        } else {
            self.tree_menu = Some(TreeMenu { x, y, on: MenuOn::Folder(folder.to_string()), confirm: true });
        }
    }

    /// Moves a folder, with every file in it, to `.trash` in the notes folder. Unsaved notes
    /// go to disk first, so the trash holds their last text.
    pub fn delete_folder(&mut self, folder: &str) {
        self.remove_folder(folder);
        self.sync_after_save();
    }

    /// `delete_folder` without the sync after it, for a delete that came from a sync.
    pub(crate) fn remove_folder(&mut self, folder: &str) {
        if folder.is_empty() {
            return;
        }
        let pre = format!("{folder}/");
        let inside = |p: &str| p == folder || p.starts_with(&pre);
        self.flush_notes();
        let Some(v) = &mut self.vault else { return };
        let root = v.path.clone();
        let gone: Vec<String> = v.files.keys().filter(|p| inside(p)).cloned().collect();
        for p in &gone {
            v.files.remove(p);
            v.born.remove(p);
        }
        v.folders.retain(|f| !inside(f));
        self.writer.send(Op::DeleteFolder { root, rel: folder.to_string() });
        for p in &gone {
            self.close_tab(p);
            self.edits.remove(p);
        }
        self.recent.retain(|p| !inside(p));
        self.pinned.retain(|p| !inside(p));
        self.unlocked.retain(|p, _| !inside(p));
        self.expanded.retain(|p| !inside(p));
        if self.new_folder.as_ref().is_some_and(|nf| inside(&nf.parent)) {
            self.new_folder = None;
        }
        if self.active.as_deref().is_some_and(inside) {
            self.active = None;
        }
        self.rebuild_names_pub();
        self.persist_later();
    }

    /// Moves a note to `.trash` in the folder, open or not.
    pub fn delete_path(&mut self, p: &str) {
        let was = self.active.clone();
        self.active = Some(p.to_string());
        self.delete_note();
        if was.as_deref() != Some(p) {
            self.active = was.filter(|a| self.files().contains_key(a));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_paths_name_their_folder() {
        assert_eq!(folder_at("root/body/sidebar/side-scroll/side-list/tree/dir:Docs|Specs/#2"), Some("Docs/Specs".into()));
        assert_eq!(folder_at("root/body/sidebar/side-scroll/side-list/tree/file:Journal|2026.md/#1"), Some("Journal".into()));
        assert_eq!(folder_at("root/body/sidebar/side-scroll/side-list"), Some(String::new()));
        assert_eq!(folder_at("root/body/sheet/doc-scroll"), None);
    }

    #[test]
    fn paths_split_into_folder_and_name() {
        assert_eq!(split_path("Drafts/Ideas/A: b"), ("Drafts/Ideas".into(), "A- b".into()));
        assert_eq!(split_path("../x"), (String::new(), "x".into()));
    }

    #[test]
    fn a_folder_click_or_link_makes_its_note() {
        use crate::app::Mode;
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let keys = ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"];
        let old: Vec<_> = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
        let dir = std::env::temp_dir().join(format!("quire-folder-note-{}", std::process::id()));
        let vault = dir.join("vault");
        std::fs::create_dir_all(vault.join("Projects")).unwrap();
        std::fs::create_dir_all(vault.join("Areas")).unwrap();
        std::fs::write(vault.join("Projects/Plan.md"), "# Plan\n").unwrap();
        std::fs::write(vault.join("Areas/Plan.md"), "# Plan\n").unwrap();
        // SAFETY: ENV keeps these variables to this test.
        unsafe {
            std::env::set_var("HOME", dir.join("home"));
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        s.load_vault(&vault).unwrap();
        let mut tx = Tx::default();
        s.open_link("Projects", &mut tx);
        assert_eq!(s.active.as_deref(), Some("Projects/Projects.md"));
        assert_eq!(s.mode, Mode::Source);
        assert!(s.files().contains_key("Projects/Projects.md"));
        s.open_folder("Areas", &mut tx);
        assert_eq!(s.active.as_deref(), Some("Areas/Areas.md"));
        assert!(!s.expanded.contains("Areas"), "opening a folder's note leaves the folder closed");
        assert!(!s.expanded.contains("Projects"));
        s.toggle_folder("Areas");
        s.open_folder("Areas", &mut tx);
        assert!(s.expanded.contains("Areas"), "and an open folder stays open");
        s.open_file("Areas/Plan.md", None, &mut tx);
        assert!(s.expanded.contains("Areas"), "a note inside a folder still opens it");
        assert!(s.files().contains_key("Areas/Areas.md"));
        assert_eq!(s.folder_note("Projects").as_deref(), Some("Projects/Projects.md"));
        s.mode = Mode::Preview;
        s.open_folder("Projects", &mut tx);
        assert_eq!(s.active.as_deref(), Some("Projects/Projects.md"));
        assert_eq!(s.mode, Mode::Preview, "an existing folder note keeps the preview on");
        let hit = crate::app::Sel { start: 2, end: 6, line: Some(0), force_source: false };
        s.open_file("Areas/Plan.md", Some(hit), &mut tx);
        assert_eq!(s.mode, Mode::Preview, "so does a search result");
        drop(s);
        unsafe {
            for (k, v) in old {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_note_icon_renames_the_note_and_keeps_links() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let keys = ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"];
        let old: Vec<_> = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
        let dir = std::env::temp_dir().join(format!("quire-note-icon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let vault = dir.join("vault");
        std::fs::create_dir_all(vault.join("🎉 Party")).unwrap();
        std::fs::write(vault.join("Wiki.md"), "# Wiki\n").unwrap();
        std::fs::write(vault.join("🎉 Party/Party.md"), "# Party\n").unwrap();
        std::fs::write(vault.join("Root.md"), "[[Wiki]]\n").unwrap();
        // SAFETY: ENV keeps these variables to this test.
        unsafe {
            std::env::set_var("HOME", dir.join("home"));
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        s.load_vault(&vault).unwrap();
        let mut tx = Tx::default();
        s.open_link("Wiki", &mut tx);

        s.start_note_icon("Wiki.md", &mut tx);
        s.apply_folder_icon(Some("🧭"), &mut tx);
        s.writer.wait(std::time::Duration::from_secs(5));
        assert!(vault.join("🧭 Wiki.md").is_file(), "the note was renamed");
        assert!(!vault.join("Wiki.md").exists());
        assert_eq!(s.active.as_deref(), Some("🧭 Wiki.md"));
        assert_eq!(s.files()["Root.md"], "[[Wiki]]\n", "a link by name stays as it is");
        assert_eq!(s.names.resolve("Wiki"), Some("🧭 Wiki.md"), "and still reaches the note");

        s.start_note_icon("🧭 Wiki.md", &mut tx);
        s.apply_folder_icon(Some("📚"), &mut tx);
        s.start_note_icon("📚 Wiki.md", &mut tx);
        s.apply_folder_icon(None, &mut tx);
        s.writer.wait(std::time::Duration::from_secs(5));
        assert!(vault.join("Wiki.md").is_file(), "the icon came off again");
        assert_eq!(s.active.as_deref(), Some("Wiki.md"));

        // A folder note shows its folder's icon, so its sticker changes the folder's.
        s.start_note_icon("🎉 Party/Party.md", &mut tx);
        assert!(s.folder_icon.as_ref().is_some_and(|p| p.note.is_none() && p.folder == "🎉 Party"));
        s.close_folder_icon(&mut tx);

        drop(s);
        unsafe {
            for (k, v) in old {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_icon_renames_the_folder_and_keeps_links() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let keys = ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"];
        let old: Vec<_> = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
        let dir = std::env::temp_dir().join(format!("quire-folder-icon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let vault = dir.join("vault");
        std::fs::create_dir_all(vault.join("Party")).unwrap();
        std::fs::write(vault.join("Party/Plan.md"), "# Plan\n").unwrap();
        std::fs::write(vault.join("Root.md"), "# Root\n\n[[Party/Plan]] and [[Party]].\n").unwrap();
        // SAFETY: ENV keeps these variables to this test.
        unsafe {
            std::env::set_var("HOME", dir.join("home"));
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        s.load_vault(&vault).unwrap();
        let mut tx = Tx::default();

        s.start_folder_icon("Party", &mut tx);
        s.apply_folder_icon(Some("🎉"), &mut tx);
        s.writer.wait(std::time::Duration::from_secs(5));
        assert!(vault.join("🎉 Party/Plan.md").is_file(), "the folder and its notes moved");
        assert!(!vault.join("Party").exists());
        assert!(s.files().contains_key("🎉 Party/Plan.md"));
        assert!(s.files()["Root.md"].contains("[[🎉 Party/Plan]]"));
        assert!(s.files()["Root.md"].contains("[[Party]]"), "a bare link keeps its name");

        s.open_link("Party", &mut tx);
        assert_eq!(s.active.as_deref(), Some("🎉 Party/Party.md"));
        s.flush_notes();
        s.writer.wait(std::time::Duration::from_secs(5));
        assert!(vault.join("🎉 Party/Party.md").is_file());

        s.start_folder_icon("🎉 Party", &mut tx);
        s.apply_folder_icon(None, &mut tx);
        s.writer.wait(std::time::Duration::from_secs(5));
        assert!(vault.join("Party/Plan.md").is_file(), "the folder moved back");
        assert!(vault.join("Party/Party.md").is_file(), "the folder note stayed with it");
        assert!(s.files()["Root.md"].contains("[[Party/Plan]]"));

        drop(s);
        unsafe {
            for (k, v) in old {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn todays_entry_goes_in_the_journal_folder_with_an_icon() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let keys = ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"];
        let old: Vec<_> = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
        let dir = std::env::temp_dir().join(format!("quire-journal-icon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let vault = dir.join("vault");
        std::fs::create_dir_all(vault.join("Journal")).unwrap();
        std::fs::write(vault.join("Journal/2020-01-01.md"), "# Old\n").unwrap();
        // SAFETY: ENV keeps these variables to this test.
        unsafe {
            std::env::set_var("HOME", dir.join("home"));
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        s.load_vault(&vault).unwrap();
        let mut tx = Tx::default();

        s.start_folder_icon("Journal", &mut tx);
        s.apply_folder_icon(Some("📓"), &mut tx);
        s.writer.wait(std::time::Duration::from_secs(5));
        assert!(vault.join("📓 Journal/2020-01-01.md").is_file());

        s.open_today(&mut tx);
        let active = s.active.clone().unwrap();
        assert!(active.starts_with("📓 Journal/"), "today's entry is {active}");
        s.flush_notes();
        s.writer.wait(std::time::Duration::from_secs(5));
        assert!(vault.join(&active).is_file(), "today's entry is on disk");
        assert!(!vault.join("Journal").exists(), "no second Journal folder");
        assert!(!s.folder_list().iter().any(|f| f == "Journal"));

        // The calendar: a click on a day with no entry makes one and opens it to write in; an
        // existing entry opens in the mode the app is in.
        use crate::app::Mode;
        use crate::calendar::Day;
        let day = Day::new(2026, 2, 3);
        assert!(!s.journal_days(2026, 2).contains(&3));
        s.mode = Mode::Preview;
        s.open_day(day, &mut tx);
        let p = s.active.clone().unwrap();
        assert_eq!(p, "📓 Journal/2026-02-03.md");
        assert_eq!(s.files()[&p], "# Tuesday 3 February\n\n#journal\n\n");
        assert_eq!(s.mode, Mode::Source, "a new entry opens in the editor");
        assert_eq!(s.note_day(&p), Some(day));
        assert_eq!(s.journal_days(2026, 2).into_iter().collect::<Vec<_>>(), vec![3]);
        s.open_file("📓 Journal/2020-01-01.md", None, &mut tx);
        assert_eq!(s.note_day("📓 Journal/2020-01-01.md"), Some(Day::new(2020, 1, 1)));
        s.mode = Mode::Preview;
        s.open_day(day, &mut tx);
        assert_eq!(s.active.as_deref(), Some(p.as_str()));
        assert_eq!(s.mode, Mode::Preview, "an existing entry keeps the preview on");

        drop(s);
        unsafe {
            for (k, v) in old {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
