//! The right panel (outline and backlinks) and the status bar.

use rune::{Color, El, ScrollSpec, col, el, icon, row, text};

use super::V;
use super::editor::{group, link_at_caret};
use super::widgets::{CHEVRON_RIGHT, label};
use crate::calendar::{self, Day};
use crate::app::{App, Mode};
use crate::keys::mod_click;
use crate::md;
use crate::theme::{mono, sans};

pub fn outline(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let doc = s.active_text();
    let heads = md::headings(doc);
    let caret = s.active.as_ref().and_then(|a| s.edits.get(a)).map_or(0, |e| e.focus.min(doc.len()));
    let (line, _) = md::line_col(doc, caret);
    let cur = if s.mode == Mode::Source {
        heads.iter().rposition(|h| h.line <= line)
    } else if heads.is_empty() {
        None
    } else {
        Some(v.prev_head.min(heads.len() - 1))
    };
    let min_lv = heads.iter().map(|h| h.level).min().unwrap_or(6);
    let mut c = col()
        .key("outline-list")
        .w_full()
        .pt(24.0)
        .px(10.0)
        .pb(24.0)
        .child(calendar_panel(v))
        .child(text("outline").font(mono(11.0, 400.0)).color(p.mute).px(10.0).pt(22.0).pb(8.0));
    for (i, h) in heads.iter().enumerate() {
        let on = cur == Some(i);
        let line = h.line;
        c = c.child(
            row()
                .key(format!("ol-{i}"))
                .gap(8.0)
                .pt(4.0)
                .pb(4.0)
                .pr(8.0)
                .pl(8.0 + (h.level - min_lv) as f32 * 14.0)
                .center_y()
                .radius(6.0)
                .hover(p.hover)
                .role("button")
                .on_press_tx(move |s: &mut App, tx| s.jump(line, tx))
                .child(el().key("dot").size(5.0, 5.0).radius(2.5).fill(if on { p.acc } else { Color::TRANSPARENT }))
                .child(text(h.text.clone()).font(sans(13.0, if on { 500.0 } else { 400.0 }).lh(13.0 * 1.35)).color(if on { p.ink } else { p.ink2 }).truncate().shrink().min_w(0.0)),
        );
    }
    if heads.is_empty() {
        c = c.child(label("No headings in this note.", sans(12.5, 400.0), p.mute).py(2.0).px(10.0));
    }
    c = c.child(text("linked from").font(mono(11.0, 400.0)).color(p.mute).pt(26.0).px(10.0).pb(8.0));
    let mut back = s.active.as_ref().map(|a| md::backlinks(s.files(), a)).unwrap_or_default();
    if let Some(vault) = &s.vault {
        back.sort_by_key(|(p, _)| (vault.born.get(p).copied().unwrap_or(u128::MAX), p.clone()));
    }
    let name_style = sans(13.0, 400.0);
    let small = mono(10.5, 400.0);
    let shift = super::baseline_shift(name_style, small);
    for (i, (bp, _)) in back.iter().enumerate() {
        let path = bp.clone();
        c = c.child(
            row()
                .key(format!("bl-{i}"))
                .gap(8.0)
                .py(4.0)
                .px(10.0)
                .radius(6.0)
                .hover(p.hover)
                .hover_group()
                .role("button")
                .on_press_tx(move |s: &mut App, tx| s.open_file(&path, None, tx))
                .child(text(md::base(bp)).font(name_style).color(p.ink2).group_color(p.ink).truncate().shrink().min_w(0.0))
                .child(text(md::dir(bp)).font(small).color(p.mute).truncate().grow(1.0).shrink().min_w(0.0).mt(shift)),
        );
    }
    if back.is_empty() {
        c = c.child(text("No notes link here yet.").font(sans(12.5, 400.0).lh(12.5 * 1.45)).color(p.mute).py(2.0).px(10.0));
    }
    col().key("outline").w(236.0).h_full().min_h(0.0).border_sides([0.0, 0.0, 0.0, 1.0], p.line).border_layout().scroll(ScrollSpec { stick_bottom: false }).child(c)
}

/// A month of the journal: a dot marks a day with an entry, an accent ring marks today, and
/// the open entry's day is filled. A click opens that day's entry, making it when needed.
fn calendar_panel(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let today = Day::today();
    let open = s.active.as_deref().and_then(|a| s.note_day(a));
    let (year, month) = s.cal_month.or(open.map(|d| (d.year, d.month))).unwrap_or((today.year, today.month));
    let have = s.journal_days(year, month);

    let step = |key: &str, by: i32, help: &str| {
        let mut chev = icon(&CHEVRON_RIGHT, 8.0).color(p.mute);
        if by < 0 {
            chev = chev.rotate(std::f32::consts::PI);
        }
        row()
            .key(key)
            .size(22.0, 22.0)
            .center()
            .radius(5.0)
            .hover(p.hover)
            .role("button")
            .label(help)
            .help(help)
            .on_press(move |s: &mut App| s.cal_month = Some(calendar::add_months(year, month, by)))
            .child(chev)
    };
    let head = row()
        .key("cal-head")
        .w_full()
        .px(4.0)
        .pb(6.0)
        .gap(2.0)
        .center_y()
        .child(text(format!("{} {year}", calendar::MONTHS[(month - 1) as usize])).font(sans(13.0, 500.0)).color(p.ink).grow(1.0).px(6.0))
        .child(step("cal-prev", -1, "Previous month"))
        .child(
            row()
                .key("cal-today")
                .h(22.0)
                .px(7.0)
                .center()
                .radius(5.0)
                .hover(p.hover)
                .role("button")
                .label("Open today's journal entry")
                .help("Open today's journal entry")
                .on_press_tx(|s: &mut App, tx| {
                    s.cal_month = None;
                    s.open_today(tx);
                })
                .child(label("Today", sans(12.0, 500.0), p.ink2)),
        )
        .child(step("cal-next", 1, "Next month"));

    let cell_h = 28.0;
    let mut weekdays = row().key("cal-days").w_full().px(4.0);
    for (i, d) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
        weekdays = weekdays.child(el().key(format!("wd-{i}")).grow(1.0).w(0.0).h(18.0).center().child(text(*d).font(mono(10.5, 400.0)).color(p.mute)));
    }
    let mut grid = col().key("cal-grid").w_full().px(4.0);
    for (wi, week) in calendar::grid(year, month).into_iter().enumerate() {
        let mut r = row().key(format!("cal-w{wi}")).w_full();
        for (di, cell) in week.into_iter().enumerate() {
            let slot = el().key(format!("c{di}")).grow(1.0).w(0.0).h(cell_h).center();
            let Some(d) = cell else {
                r = r.child(slot);
                continue;
            };
            let day = Day::new(year, month, d);
            let is_today = day == today;
            let is_open = open == Some(day);
            let has = have.contains(&d);
            let mut cell = col()
                .key("day")
                .size(26.0, 26.0)
                .center()
                .radius(7.0)
                .hover(p.hover)
                .role("button")
                .label(format!("{}{}", day.long(), if has { ", has an entry" } else { "" }))
                .help(day.long())
                .on_press_tx(move |s: &mut App, tx| s.open_day(day, tx))
                .child(text(d.to_string()).font(sans(12.0, if is_today || is_open { 600.0 } else { 400.0 }).lh(14.0)).color(if is_today { p.acc } else if has || is_open { p.ink } else { p.ink2 }))
                .child(el().key("dot").size(4.0, 4.0).mt(1.0).radius(2.0).fill(if has { if is_today { p.acc } else { p.mute } } else { Color::TRANSPARENT }));
            if is_open {
                cell = cell.fill(p.act);
            }
            if is_today {
                cell = cell.border(1.0, p.acc.alpha(0.55));
            }
            r = r.child(slot.child(cell));
        }
        grid = grid.child(r);
    }
    col()
        .key("calendar")
        .w_full()
        .child(text("calendar").font(mono(11.0, 400.0)).color(p.mute).px(10.0).pb(8.0))
        .child(head)
        .child(weekdays)
        .child(grid)
}

pub fn status(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let doc = s.active_text();
    let edit = s.active.as_ref().and_then(|a| s.edits.get(a));
    let (a, b) = edit.map_or((0, 0), |e| (e.anchor.min(e.focus).min(doc.len()), e.anchor.max(e.focus).min(doc.len())));
    let pos = edit.map_or(0, |e| e.focus.min(doc.len()));
    let (line, ls) = md::line_col(doc, pos);
    let col_ = doc[ls..pos].chars().count();
    let sel = &doc[a..b];
    let style = mono(11.5, 400.0);
    let preview = s.mode == Mode::Preview;
    let pos_label = if preview {
        "preview".to_string()
    } else if !sel.is_empty() {
        format!("{} selected", group(sel.chars().count()))
    } else {
        format!("Ln {}, Col {}", line + 1, col_ + 1)
    };
    let wc = md::words(doc);
    let word_label = if !sel.is_empty() {
        format!("{} of {} words", group(md::words(sel)), group(wc))
    } else {
        format!("{} {}", group(wc), if wc == 1 { "word" } else { "words" })
    };
    let read = format!("{} min read", ((wc as f32 / 230.0).round() as usize).max(1));
    let mut r = row().key("status").h(29.0).w_full().gap(18.0).px(16.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], p.line).border_layout().clip_rounded(0.0).child(label(pos_label, style, p.mute));
    if !preview {
        if let Some(t) = link_at_caret(doc, pos) {
            let verb = if s.names.resolve(&t).is_some() { " to open “" } else { " to create “" };
            r = r.child(text(format!("{}{verb}{t}”", mod_click())).font(style).color(p.acc).truncate().shrink().min_w(0.0));
        }
    }
    r = r.child(el().grow(1.0)).child(label(word_label, style, p.mute)).child(label(read, style, p.mute));
    if let Some((lbl, busy, failed)) = s.sync_label() {
        let dot = if failed { p.err } else if busy { p.acc } else { p.mark };
        let mut sync = row()
            .key("sync-state")
            .role("button")
            .on_press(|s: &mut App| {
                s.open_settings();
                s.set_sec = crate::app::Sec::Sync;
            })
            .gap(6.0)
            .center_y().child(el().size(6.0, 6.0).radius(3.0).fill(dot)).child(label(lbl, style, if failed { p.err } else { p.mute }));
        if let Some(e) = &s.sync.error {
            sync = sync.help(e.clone());
        }
        r = r.child(sync);
    }
    r
}
