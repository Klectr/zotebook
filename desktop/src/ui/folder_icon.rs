//! The dialog that gives a folder an emoji icon. The icon is the first character of the folder
//! name, so the picker renames the folder when it applies one.

use rune::{Color, El, ScrollSpec, col, el, row, text, text_input};

use super::V;
use super::widgets::{label, outline_button, primary_button};
use crate::app::App;
use crate::emoji;
use crate::md;
use crate::theme::{self, mono, sans};

/// A text field styled like the settings fields.
fn field(v: V, key: &'static str, edit: &rune::TextEdit, bad: bool, ph: &'static str, style: rune::TextStyle) -> El {
    let p = v.p;
    row()
        .key(format!("box-{key}"))
        .h(34.0)
        .min_w(0.0)
        .center_y()
        .radius(7.0)
        .fill(p.bg)
        .border(1.0, if bad { p.err } else { p.line })
        .border_layout()
        .clip_rounded(7.0)
        .child(
            text_input(edit, style)
                .key(key)
                .single_line()
                .grow(1.0)
                .min_w(0.0)
                .px(10.0)
                .color(p.ink)
                .placeholder(ph, theme::hex("#9a9184"))
                .caret(p.ink, p.sel)
                .label(ph)
                .on_edit(move |s: &mut App| match key {
                    "folder-icon-search" => &mut s.folder_icon.as_mut().unwrap().search,
                    _ => &mut s.folder_icon.as_mut().unwrap().custom,
                })
                .on_input_change(|s: &mut App, _| {
                    if let Some(p) = &mut s.folder_icon {
                        p.error = None;
                    }
                })
                .on_submit(move |s: &mut App, tx| {
                    if key == "folder-icon-search" {
                        s.apply_searched_folder_icon(tx);
                    } else {
                        s.apply_custom_folder_icon(tx);
                    }
                }),
        )
}

pub fn dialog(v: V) -> El {
    let p = v.p;
    let Some(picker) = &v.s.folder_icon else { return el() };
    let folder = picker.folder.clone();
    let display = md::folder_name(&folder);
    let display = if display.is_empty() { folder.clone() } else { display.to_string() };
    let current = md::folder_icon(&folder);
    let results = emoji::search(&picker.search.text);
    let error = picker.error.clone();

    let mut head = row().key("folder-icon-head").w_full().gap(12.0).center_y();
    if let Some(icon) = current {
        head = head.child(
            row()
                .size(38.0, 38.0)
                .center()
                .radius(9.0)
                .fill(p.soft)
                .child(text(icon).font(sans(20.0, 400.0)).color(p.ink)),
        );
    }
    head = head.child(
        col()
            .gap(3.0)
            .grow(1.0)
            .min_w(0.0)
            .child(label("Folder icon", sans(16.0, 600.0), p.ink))
            .child(label(format!("Choose an emoji for “{display}”."), sans(12.5, 400.0).lh(12.5 * 1.45), p.mute).wrap()),
    );

    let search = field(v, "folder-icon-search", &picker.search, false, "Search emoji", sans(13.5, 400.0)).w_full();

    let mut grid = col().key("folder-icon-grid").w_full().gap(2.0).padding(4.0);
    for (ri, chunk) in results.chunks(8).enumerate() {
        let mut r = row().key(format!("folder-icon-row-{ri}")).gap(2.0);
        for (ci, e) in chunk.iter().enumerate() {
            let i = ri * 8 + ci;
            let e = *e;
            let on = current == Some(e);
            r = r.child(
                row()
                    .key(format!("folder-icon-{i}"))
                    .size(36.0, 36.0)
                    .center()
                    .radius(7.0)
                    .fill(if on { p.act } else { Color::TRANSPARENT })
                    .hover(p.hover)
                    .role("button")
                    .label(format!("Use {e}"))
                    .on_press_tx(move |s: &mut App, tx| s.apply_folder_icon(Some(e), tx))
                    .child(text(e).font(sans(20.0, 400.0)).color(p.ink)),
            );
        }
        grid = grid.child(r);
    }
    if results.is_empty() {
        grid = grid.child(label("No emoji matches.", sans(13.0, 400.0), p.mute).px(8.0).py(14.0));
    }
    let scroller = col().key("folder-icon-scroll").w_full().max_h(232.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(grid);

    let custom = row()
        .key("folder-icon-custom-row")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(label("Any emoji", sans(12.5, 500.0), p.ink2))
        .child(field(v, "folder-icon-custom", &picker.custom, error.is_some(), "Type or paste", sans(16.0, 400.0)).w(148.0))
        .child(el().grow(1.0))
        .child(primary_button(p, "folder-icon-apply", "Apply", 32.0, 12.0, 13.0, |s, tx| s.apply_custom_folder_icon(tx)));

    let mut panel = col()
        .key("folder-icon-dialog")
        .w(456.0)
        .padding(20.0)
        .gap(12.0)
        .radius(12.0)
        .fill(p.bg)
        .box_shadow(theme::settings_shadows()[0])
        .box_shadow(theme::settings_shadows()[1])
        .role("dialog")
        .label("Folder icon")
        .modal_tx(true, |s: &mut App, tx| s.close_folder_icon(tx))
        .child(head)
        .child(search)
        .child(scroller)
        .child(
            row()
                .key("folder-icon-sep")
                .w_full()
                .pt(12.0)
                .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
                .border_layout(),
        )
        .child(custom);
    if let Some(e) = error {
        panel = panel.child(label(e, sans(12.5, 400.0), p.err).wrap());
    }
    let mut foot = row().key("folder-icon-foot").w_full().gap(8.0).center_y();
    if current.is_some() && !display.is_empty() {
        foot = foot.child(outline_button(p, "folder-icon-reset", "Use default icon", 32.0, |s, tx| s.apply_folder_icon(None, tx)));
    }
    panel = panel.child(
        foot.child(el().grow(1.0))
            .child(label("esc closes", mono(11.0, 400.0), p.mute))
            .child(outline_button(p, "folder-icon-cancel", "Cancel", 32.0, |s, tx| s.close_folder_icon(tx))),
    );
    el().key("folder-icon-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).paint_order(65).fill(p.scrim_settings).center().child(panel)
}
