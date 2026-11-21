//! Settings: Sync. Notes always live on this computer. Local leaves them there; Git commits the
//! notes folder and pushes it once "Test connection" passes, then on save, every 5 minutes or
//! by hand. The object-storage engine in `crate::sync` stays for hosted sync.

use rune::{Color, El, TextEdit, col, row, text, text_input};

use super::V;
use super::settings::{block, body_width, desc_text, row_};
use super::widgets::{label, primary_button, seg, settings_seg};
use crate::app::{App, SyncStatus};
use crate::theme::{self, mono, sans};

/// A sync choice in the settings.
pub struct Choice {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
}

/// The choices, in the order `store::SYNC_CHOICES` lists them.
pub const CHOICES: [Choice; 2] = [
    Choice { id: "none", name: "Local", kind: "on this computer" },
    Choice { id: "github", name: "Git", kind: "commit and push" },
];

/// A field of the Git form: the notes folder, shown read-only, and the remote.
struct Field {
    key: &'static str,
    label: &'static str,
    read: bool,
    def: &'static str,
    hint: &'static str,
    value: String,
}

fn git_fields(s: &App) -> Vec<Field> {
    let folder = s.vault.as_ref().map_or(String::new(), |v| v.path.display().to_string());
    vec![
        Field { key: "folder", label: "Notes folder", read: true, def: "", hint: "", value: folder },
        Field { key: "remote", label: "Remote", read: false, def: "origin", hint: "the Git remote, usually origin", value: String::new() },
    ]
}

fn cfg_value(s: &App, pid: &str, key: &str) -> Option<String> {
    s.settings().sync.cfg.get(pid).and_then(|c| c.get(key)).cloned()
}

pub fn set_cfg(s: &mut App, key: &str, v: String) {
    let pid = s.settings().sync.provider.clone();
    s.settings_mut().sync.cfg.entry(pid).or_default().insert(key.to_string(), v);
    s.sync_changed();
    s.sync_status = match std::mem::replace(&mut s.sync_status, SyncStatus::Idle) {
        SyncStatus::Error { msg, missing } => SyncStatus::Error { msg, missing: missing.into_iter().filter(|m| m != key).collect() },
        _ => SyncStatus::Idle,
    };
}

pub fn sync(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let sy = &s.settings().sync;
    let pid = sy.provider.clone();
    let w = body_width(v);
    let card_w = (w - 8.0) / 2.0;
    let mut cards = row().key("choices").w_full().gap(8.0).mt(14.0);
    for ch in &CHOICES {
        let on = ch.id == pid;
        let id = ch.id;
        let mut card = col()
            .key(format!("pv-{id}"))
            .w(card_w)
            .gap(3.0)
            .py(10.0)
            .px(12.0)
            .radius(9.0)
            .fill(p.bg)
            .border(1.0, if on { p.acc } else { p.line })
            .border_layout()
            .role("radio")
            .checked(on)
            .label(ch.name)
            .on_press(move |s: &mut App| {
                if s.settings().sync.provider != id {
                    s.settings_mut().sync.provider = id.into();
                    s.sync_changed();
                    s.sync_status = SyncStatus::Idle;
                }
            })
            .child(text(ch.name).font(sans(13.0, 500.0)).color(p.ink).truncate().max_w(card_w - 26.0))
            .child(text(ch.kind).font(mono(10.5, 400.0)).color(if on { p.acc } else { p.mute }).truncate().max_w(card_w - 26.0));
        if on {
            card = card.box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: p.acc, inset: false });
        }
        cards = cards.child(card);
    }
    let c = col()
        .key("sync-set")
        .w_full()
        .child(block(v, "backend", "How notes sync", Some("Notes always live on this computer first, as plain files. Choose how they reach your other computers.")).child(cards));
    if pid != "github" {
        return c.child(
            col()
                .key("local-note")
                .w_full()
                .gap(3.0)
                .py(16.0)
                .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
                .border_layout()
                .child(label("Use it on other computers", sans(14.0, 500.0), p.ink))
                .child(desc_text(v, "Keep this notes folder in iCloud Drive, Dropbox or OneDrive, and it syncs like any other folder there.")),
        );
    }
    let missing: Vec<String> = match &s.sync_status {
        SyncStatus::Error { missing, .. } => missing.clone(),
        _ => Vec::new(),
    };
    let mut form = col().key("fields").w_full().gap(14.0).mt(14.0);
    for fd in git_fields(s) {
        let bad = missing.contains(&fd.key.to_string());
        form = form.child(field(v, &pid, &fd, bad).w_full());
    }
    let mut c = c.child(col().key("provider-form").w_full().pt(16.0).pb(18.0).border_sides([1.0, 0.0, 0.0, 0.0], p.line).border_layout().child(label("Git repository", sans(14.0, 500.0), p.ink)).child(form));
    let when = |id: &'static str, l: &str| seg(l, sy.when == id, move |s, _| s.settings_mut().sync.when = id.into());
    c = c.child(row_(v, "when", "When to sync", Some("On save syncs a moment after you stop typing."), settings_seg(p, "when-seg", vec![when("save", "On save"), when("interval", "Every 5 min"), when("manual", "Manually")])));
    c = c.child(
        col()
            .key("git-note")
            .w_full()
            .gap(3.0)
            .py(16.0)
            .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
            .border_layout()
            .child(label("How it syncs", sans(14.0, 500.0), p.ink))
            .child(desc_text(
                v,
                "Zotebook commits the notes folder, rebases onto the remote branch and pushes it, using the Git credentials already on this computer. The folder must be the root of a Git repository, and Git needs a user name and email. Locked notes stay encrypted; other notes are committed as plain text, so use a private repository. Zotebook keeps .trash out of Git on this computer.",
            )),
    );
    let (msg, color) = match &s.sync_status {
        SyncStatus::Idle if sy.connected && s.sync.error.is_some() => (format!("The last sync failed. {}", s.sync.error.clone().unwrap_or_default()), p.err),
        SyncStatus::Idle if sy.connected => (
            match sy.last_sync {
                Some(t) => format!("Connected. Last synced {}.", crate::sync::app::ago(t)),
                None => "Connected. Not synced yet.".to_string(),
            },
            p.mute,
        ),
        SyncStatus::Idle => ("Not connected yet.".to_string(), p.mute),
        SyncStatus::Testing { msg } => (msg.clone(), p.mute),
        SyncStatus::Error { msg, .. } => (msg.clone(), p.err),
        SyncStatus::Ok { msg } => (msg.clone(), p.ink2),
    };
    let working = s.sync.testing() || s.sync.busy();
    c.child(
        row()
            .key("sync-actions")
            .w_full()
            .flex_wrap()
            .gap(10.0)
            .pt(16.0)
            .pb(4.0)
            .center_y()
            .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
            .border_layout()
            .child(primary_button(p, "test", if s.sync.testing() { "Working…" } else { "Test connection" }, 32.0, 14.0, 13.0, |s, _| {
                if !s.sync.testing() {
                    s.start_test();
                }
            }))
            .children(if sy.connected && !working {
                vec![super::widgets::outline_button(p, "sync-now", "Sync now", 32.0, |s, _| s.sync_now()).px(14.0)]
            } else {
                Vec::new()
            })
            .child(text(msg).font(sans(12.5, 400.0).lh(12.5 * 1.45)).color(color).wrap().grow(1.0).min_w(180.0)),
    )
}

fn field(v: V, pid: &str, fd: &Field, bad: bool) -> El {
    let p = v.p;
    let s = v.s;
    let head = row()
        .key("head")
        .w_full()
        .gap(8.0)
        .justify_between()
        .child(label(fd.label, sans(12.5, 500.0), p.ink2))
        .child(text(fd.hint).font(sans(11.5, 400.0)).color(p.mute).truncate().shrink().min_w(0.0).mt(super::baseline_shift(sans(12.5, 500.0), sans(11.5, 400.0))));
    let c = col().key(format!("f-{}", fd.key)).gap(6.0).min_w(0.0).child(head);
    if fd.read {
        return c.child(text(fd.value.clone()).font(mono(12.0, 400.0)).color(p.ink2).wrap().wrap_anywhere().w_full().py(7.0).px(10.0).radius(7.0).fill(p.code));
    }
    let key = fd.key;
    let value = cfg_value(s, pid, key).unwrap_or_else(|| fd.def.to_string());
    c.child(text_box(v, &format!("sync:{pid}:{key}"), value, fd.def, false, bad, f32::INFINITY, move |s, t| set_cfg(s, key, t)))
}

/// A 32-point single-line field with an optional show and hide button for secrets.
pub fn text_box(v: V, fk: &str, value: String, ph: &'static str, secret: bool, bad: bool, max_w: f32, set: impl Fn(&mut App, String) + 'static) -> El {
    let p = v.p;
    let s = v.s;
    let revealed = s.reveal.contains(fk);
    let mut edit = s.fields.get(fk).cloned().unwrap_or_else(|| TextEdit::new(value.clone()));
    if secret && !revealed {
        edit = edit.secure();
    }
    let fk_edit = fk.to_string();
    let fk_change = fk.to_string();
    let init = value.clone();
    let mut b = row()
        .key(format!("box-{fk}"))
        .w_full()
        .h(32.0)
        .center_y()
        .radius(7.0)
        .fill(p.bg)
        .border(1.0, if bad { p.err } else { p.line })
        .border_layout()
        .clip_rounded(7.0)
        .child(
            text_input(&edit, mono(12.5, 400.0))
                .key(fk.to_string())
                .single_line()
                .grow(1.0)
                .min_w(0.0)
                .px(10.0)
                .color(p.ink)
                .placeholder(ph, theme::hex("#9a9184"))
                .caret(p.ink, p.sel)
                .on_edit(move |s: &mut App| s.field(&fk_edit, || init.clone()))
                .on_input_change(move |s: &mut App, _| {
                    let t = s.fields.get(&fk_change).map(|e| e.text.clone()).unwrap_or_default();
                    set(s, t);
                }),
        );
    if max_w.is_finite() {
        b = b.max_w(max_w);
    }
    if secret {
        let rk = fk.to_string();
        b = b.child(
            row()
                .key("reveal")
                .h_full()
                .px(10.0)
                .center()
                .border_sides([0.0, 0.0, 0.0, 1.0], p.line)
                .border_layout()
                .hover_group()
                .role("button")
                .on_press(move |s: &mut App| {
                    if !s.reveal.remove(&rk) {
                        s.reveal.insert(rk.clone());
                    }
                    if let Some(e) = s.fields.get_mut(&rk) {
                        e.set_secure(!s.reveal.contains(&rk));
                    }
                })
                .child(label(if revealed { "hide" } else { "show" }, mono(11.0, 400.0), p.mute).group_color(p.ink)),
        );
    }
    let _ = (desc_text, Color::TRANSPARENT);
    b
}

#[cfg(test)]
mod tests {
    use rune::{Env, Runtime};

    /// The settings show Local and Git, with the hint or the Git form, and the list matches the
    /// choices the state loader keeps.
    #[test]
    fn sync_settings_offer_local_and_git() {
        assert_eq!(super::CHOICES.map(|c| c.id), crate::store::SYNC_CHOICES);
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("zotebook-syncset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        s.settings_open = true;
        s.set_sec = crate::app::Sec::Sync;
        let mut rt = Runtime::new(s, crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(1.0 / 60.0);
        assert!(rt.find("pv-none").is_some() && rt.find("pv-github").is_some());
        assert!(rt.find("pv-s3").is_none(), "no object-storage providers");
        assert!(rt.find("local-note").is_some() && rt.find("f-remote").is_none());
        let (x, y) = rt.find("pv-github").unwrap();
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        for _ in 0..3 {
            rt.frame(1.0 / 60.0);
        }
        rt.update(|s, _| assert_eq!(s.settings().sync.provider, "github"));
        assert!(rt.find("f-remote").is_some() && rt.find("f-folder").is_some() && rt.find("test").is_some());
        assert!(rt.find("local-note").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A store saved before the settings dropped object storage loads as Local.
    #[test]
    fn an_old_object_store_loads_as_local() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("zotebook-oldstore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let state = dir.join("state.json");
        let mut p = crate::store::Persist::default();
        p.settings.sync.provider = "s3".into();
        p.settings.sync.connected = true;
        crate::store::save_at(&state, &p).unwrap();
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe { std::env::set_var("ZOTEBOOK_STATE", &state) };
        let got = crate::store::load();
        unsafe { std::env::remove_var("ZOTEBOOK_STATE") };
        assert_eq!(got.settings.sync.provider, "none");
        assert!(!got.settings.sync.connected);
        p.settings.sync.provider = "github".into();
        crate::store::save_at(&state, &p).unwrap();
        unsafe { std::env::set_var("ZOTEBOOK_STATE", &state) };
        let got = crate::store::load();
        unsafe { std::env::remove_var("ZOTEBOOK_STATE") };
        assert_eq!(got.settings.sync.provider, "github", "Git stays");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
