//! Right-click menu «Вырезать / Копировать / Вставить» for every text field of the editor.
//! egui has no such menu of its own, so it is drawn once per frame from the app: it finds
//! the text field under the pointer and asks the window to cut, copy or paste in it.
//! Text selected in labels (help, log, chat) gets the same menu with «Копировать» only.

use eframe::egui::{
    self, text::CCursorRange, text_edit::TextEditState, text_selection::LabelSelectionState,
    Button, Context, Frame, Id, Key, Order, PointerButton, Popup, Pos2,
};

/// A menu that is open: the field it belongs to and where it was called.
#[derive(Clone, Copy)]
struct Open {
    field: Id,
    pos: Pos2,
    /// The field has selected text, so cut and copy have something to take.
    selected: bool,
    /// The menu is for text selected in labels, not for a text field.
    labels: bool,
}

/// The text field under the pointer, if any.
fn field_under_pointer(ctx: &Context) -> Option<Open> {
    let pos = ctx.input(|i| i.pointer.interact_pos())?;
    let ids: Vec<Id> = ctx.viewport(|v| v.hits.contains_pointer.iter().map(|w| w.id).collect());
    ids.into_iter().find_map(|field| {
        let state = TextEditState::load(ctx, field)?;
        let selected = state.cursor.char_range().is_some_and(|r| !r.is_empty());
        Some(Open {
            field,
            pos,
            selected,
            labels: false,
        })
    })
}

/// Selection of the focused text field as of the last frame without a right click.
/// egui moves the cursor on any mouse press, so a right click would drop the selection
/// before the menu could cut or copy it.
fn remember_selection(ctx: &Context, key: Id) {
    let current = ctx.memory(|m| m.focused()).and_then(|field| {
        let range = TextEditState::load(ctx, field)?.cursor.char_range()?;
        Some((field, range))
    });
    ctx.data_mut(|d| d.insert_temp(key, current));
}

/// Puts back the selection the right click has just dropped.
fn restore_selection(ctx: &Context, key: Id, field: Id) -> bool {
    let kept: Option<(Id, CCursorRange)> = ctx
        .data(|d| d.get_temp::<Option<(Id, CCursorRange)>>(key))
        .flatten();
    let Some((id, range)) = kept.filter(|(id, range)| *id == field && !range.is_empty()) else {
        return false;
    };
    let Some(mut state) = TextEditState::load(ctx, id) else {
        return false;
    };
    state.cursor.set_char_range(Some(range));
    state.store(ctx, id);
    true
}

fn menu_key() -> Id {
    Id::unique("text_menu")
}

/// Keeps the text selected in labels. egui drops it on any mouse press, the right click
/// and the click on «Копировать» included, so it is put back while the menu is open and
/// on the frame the copy arrives. Call at the start of the frame, before any label.
pub fn begin(ctx: &Context) {
    let kept_key = menu_key().with("labels");
    let restore_key = menu_key().with("restore");
    let restore = ctx.data_mut(|d| d.remove_temp::<bool>(restore_key)) == Some(true);
    let Some(plugin) = ctx.plugin_opt::<LabelSelectionState>() else {
        return;
    };
    let mut state = plugin.lock();
    if restore {
        let kept = ctx.data(|d| d.get_temp::<Option<LabelSelectionState>>(kept_key));
        if let Some(kept) = kept.flatten() {
            *state = kept;
        }
    } else {
        let kept = has_selected_text(&state).then(|| state.clone());
        ctx.data_mut(|d| d.insert_temp(kept_key, kept));
    }
}

/// Some label text is selected, not just a cursor left by a click. egui does not tell
/// this apart, so the two ends of the selection are compared through its debug form;
/// the tests check that it still works after an egui update.
fn has_selected_text(state: &LabelSelectionState) -> bool {
    let text = format!("{state:?}");
    let end = |name: &str| {
        let rest = text.split(name).nth(1)?;
        rest.split('}').next()
    };
    match (
        end("primary: WidgetTextCursor"),
        end("secondary: WidgetTextCursor"),
    ) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    }
}

/// The menu for text selected in labels, if there is such text.
fn labels_menu(ctx: &Context) -> Option<Open> {
    let kept = ctx.data(|d| d.get_temp::<Option<LabelSelectionState>>(menu_key().with("labels")));
    kept.flatten()?;
    Some(Open {
        field: Id::NULL,
        pos: ctx.input(|i| i.pointer.interact_pos())?,
        selected: true,
        labels: true,
    })
}

/// Opens the menu on a right click in a text field, draws it and runs the chosen command.
pub fn show(ctx: &Context) {
    let key = menu_key();
    let kept_key = key.with("selection");
    let mut open: Option<Open> = ctx.data(|d| d.get_temp::<Option<Open>>(key)).flatten();
    if ctx.input(|i| i.pointer.button_pressed(PointerButton::Secondary)) {
        open = field_under_pointer(ctx).or_else(|| labels_menu(ctx));
        if let Some(o) = open.as_mut().filter(|o| !o.labels) {
            if restore_selection(ctx, kept_key, o.field) {
                o.selected = true;
            }
            ctx.memory_mut(|m| m.request_focus(o.field));
        }
    } else {
        remember_selection(ctx, kept_key);
    }
    // A widget under the pointer has a menu of its own: it wins over the label one.
    if open.is_some_and(|o| o.labels) && Popup::is_any_open(ctx) {
        open = None;
    }
    let mut restore_labels = open.is_some_and(|o| o.labels);
    if let Some(o) = open {
        let mut command = None;
        let area = egui::Area::new(key.with("area"))
            .order(Order::Foreground)
            .fixed_pos(o.pos)
            .constrain(true)
            .show(ctx, |ui| {
                Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(170.0);
                    let items = [
                        (
                            crate::i18n::tr("Вырезать"),
                            "Ctrl+X",
                            o.selected,
                            egui::ViewportCommand::RequestCut,
                        ),
                        (
                            crate::i18n::tr("Копировать"),
                            "Ctrl+C",
                            o.selected,
                            egui::ViewportCommand::RequestCopy,
                        ),
                        (
                            crate::i18n::tr("Вставить"),
                            "Ctrl+V",
                            true,
                            egui::ViewportCommand::RequestPaste,
                        ),
                    ];
                    let items = if o.labels { &items[1..2] } else { &items[..] };
                    for &(text, shortcut, enabled, ref cmd) in items {
                        let button = Button::new(text).shortcut_text(shortcut).frame(false);
                        if ui.add_enabled(enabled, button).clicked() {
                            command = Some(cmd.clone());
                        }
                    }
                });
            });
        let outside_click = ctx.input(|i| {
            i.pointer.primary_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !area.response.rect.contains(p))
        });
        if let Some(cmd) = command {
            // The window answers with a cut/copy/paste event for the focused field
            // or the selected labels.
            if !o.labels {
                ctx.memory_mut(|m| m.request_focus(o.field));
            }
            ctx.send_viewport_cmd(cmd);
            ctx.request_repaint();
            open = None;
        } else if outside_click || ctx.input(|i| i.key_pressed(Key::Escape)) {
            open = None;
            restore_labels = false;
        }
    }
    if restore_labels {
        ctx.data_mut(|d| d.insert_temp(key.with("restore"), true));
    }
    ctx.data_mut(|d| d.insert_temp(key, open));
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{vec2, Event, Modifiers, RawInput, Rect, TextEdit};

    fn frame_with(
        ctx: &Context,
        text: &mut String,
        events: Vec<Event>,
        select: bool,
    ) -> Option<String> {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            begin(ui.ctx());
            let id = Id::unique("field");
            ui.add(TextEdit::singleline(text).id(id));
            if select {
                let mut state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
                state.cursor.set_char_range(Some(CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(5),
                )));
                state.store(ui.ctx(), id);
                ui.memory_mut(|m| m.request_focus(id));
            }
            ui.label("selectable label text");
            show(ui.ctx());
        });
        // Nothing paints here; the font texture deltas must still be dropped explicitly.
        output.textures_delta.clear();
        output
            .platform_output
            .commands
            .iter()
            .find_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
    }

    fn frame(ctx: &Context, text: &mut String, events: Vec<Event>) -> Option<String> {
        frame_with(ctx, text, events, false)
    }

    fn is_open(ctx: &Context) -> bool {
        let state = ctx.data(|d| d.get_temp::<Option<Open>>(Id::unique("text_menu")));
        state.flatten().is_some()
    }

    #[test]
    fn right_click_in_text_field_opens_menu() {
        let ctx = Context::default();
        let mut text = String::from("hello");
        let at = Pos2::new(20.0, 15.0);
        frame(&ctx, &mut text, vec![]);
        frame(&ctx, &mut text, vec![Event::PointerMoved(at)]);
        assert!(!is_open(&ctx));
        let click = Event::PointerButton {
            pos: at,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut text, vec![click]);
        assert!(is_open(&ctx));
    }

    #[test]
    fn right_click_elsewhere_opens_nothing() {
        let ctx = Context::default();
        let mut text = String::from("hello");
        let at = Pos2::new(300.0, 250.0);
        frame(&ctx, &mut text, vec![]);
        frame(&ctx, &mut text, vec![Event::PointerMoved(at)]);
        let click = Event::PointerButton {
            pos: at,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut text, vec![click]);
        assert!(!is_open(&ctx));
    }

    #[test]
    fn right_click_keeps_the_selection() {
        let ctx = Context::default();
        let mut text = String::from("hello");
        let at = Pos2::new(20.0, 15.0);
        frame(&ctx, &mut text, vec![]);
        frame(&ctx, &mut text, vec![Event::PointerMoved(at)]);
        frame_with(&ctx, &mut text, vec![], true);
        frame(&ctx, &mut text, vec![]);
        let click = Event::PointerButton {
            pos: at,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut text, vec![click]);
        frame(&ctx, &mut text, vec![]);
        let field = Id::unique("field");
        let state = TextEditState::load(&ctx, field).expect("field state");
        assert!(state.cursor.char_range().is_some_and(|r| !r.is_empty()));
        let open = ctx.data(|d| d.get_temp::<Option<Open>>(Id::unique("text_menu")));
        assert!(open.flatten().is_some_and(|o| o.selected));
    }

    fn button(at: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn label_menu_open(ctx: &Context) -> bool {
        let open = ctx.data(|d| d.get_temp::<Option<Open>>(Id::unique("text_menu")));
        open.flatten().is_some_and(|o| o.labels)
    }

    /// Presses the primary button at `from`, drags to `to` and releases.
    fn drag(ctx: &Context, text: &mut String, from: Pos2, to: Pos2) {
        frame(ctx, text, vec![Event::PointerMoved(from)]);
        frame(ctx, text, vec![button(from, PointerButton::Primary, true)]);
        frame(ctx, text, vec![Event::PointerMoved(to)]);
        frame(ctx, text, vec![button(to, PointerButton::Primary, false)]);
        frame(ctx, text, vec![]);
    }

    fn right_click(ctx: &Context, text: &mut String, at: Pos2) {
        frame(ctx, text, vec![button(at, PointerButton::Secondary, true)]);
        frame(ctx, text, vec![button(at, PointerButton::Secondary, false)]);
    }

    // The label is the second row, under the text field.
    const LABEL_FROM: Pos2 = Pos2::new(12.0, 32.0);
    const LABEL_TO: Pos2 = Pos2::new(90.0, 32.0);

    #[test]
    fn right_click_keeps_label_selection_for_copy() {
        let ctx = Context::default();
        let mut text = String::from("hello");
        frame(&ctx, &mut text, vec![]);
        drag(&ctx, &mut text, LABEL_FROM, LABEL_TO);
        right_click(&ctx, &mut text, LABEL_TO);
        assert!(label_menu_open(&ctx));
        // «Копировать» is the only item, right under the pointer; the press on it is
        // a click outside the label, which drops the selection in egui.
        let item = LABEL_TO + egui::vec2(30.0, 12.0);
        frame(&ctx, &mut text, vec![Event::PointerMoved(item)]);
        frame(
            &ctx,
            &mut text,
            vec![button(item, PointerButton::Primary, true)],
        );
        frame(
            &ctx,
            &mut text,
            vec![button(item, PointerButton::Primary, false)],
        );
        assert!(
            !label_menu_open(&ctx),
            "the click on the item closes the menu"
        );
        // The window answers RequestCopy with a copy event.
        let copied = frame(&ctx, &mut text, vec![Event::Copy]);
        assert!(
            copied.is_some_and(|t| !t.is_empty() && "selectable label text".contains(&t)),
            "the selection made before the right click is copied"
        );
    }

    #[test]
    fn click_without_selection_opens_no_label_menu() {
        let ctx = Context::default();
        let mut text = String::from("hello");
        frame(&ctx, &mut text, vec![]);
        drag(&ctx, &mut text, LABEL_FROM, LABEL_FROM);
        right_click(&ctx, &mut text, LABEL_FROM);
        assert!(!label_menu_open(&ctx));
    }
}
