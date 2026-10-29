//! Right-click menu «Вырезать / Копировать / Вставить» for every text field of the editor.
//! egui has no such menu of its own, so it is drawn once per frame from the app: it finds
//! the text field under the pointer and asks the window to cut, copy or paste in it.

use eframe::egui::{
    self, text::CCursorRange, text_edit::TextEditState, Button, Context, Frame, Id, Key, Order,
    PointerButton, Pos2,
};

/// A menu that is open: the field it belongs to and where it was called.
#[derive(Clone, Copy)]
struct Open {
    field: Id,
    pos: Pos2,
    /// The field has selected text, so cut and copy have something to take.
    selected: bool,
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

/// Opens the menu on a right click in a text field, draws it and runs the chosen command.
pub fn show(ctx: &Context) {
    let key = Id::unique("text_menu");
    let kept_key = key.with("selection");
    let mut open: Option<Open> = ctx.data(|d| d.get_temp::<Option<Open>>(key)).flatten();
    if ctx.input(|i| i.pointer.button_pressed(PointerButton::Secondary)) {
        open = field_under_pointer(ctx);
        if let Some(o) = &mut open {
            if restore_selection(ctx, kept_key, o.field) {
                o.selected = true;
            }
            ctx.memory_mut(|m| m.request_focus(o.field));
        }
    } else {
        remember_selection(ctx, kept_key);
    }
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
                            "Вырезать",
                            "Ctrl+X",
                            o.selected,
                            egui::ViewportCommand::RequestCut,
                        ),
                        (
                            "Копировать",
                            "Ctrl+C",
                            o.selected,
                            egui::ViewportCommand::RequestCopy,
                        ),
                        (
                            "Вставить",
                            "Ctrl+V",
                            true,
                            egui::ViewportCommand::RequestPaste,
                        ),
                    ];
                    for (text, shortcut, enabled, cmd) in items {
                        let button = Button::new(text).shortcut_text(shortcut).frame(false);
                        if ui.add_enabled(enabled, button).clicked() {
                            command = Some(cmd);
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
            // The window answers with a cut/copy/paste event for the focused field.
            ctx.memory_mut(|m| m.request_focus(o.field));
            ctx.send_viewport_cmd(cmd);
            open = None;
        } else if outside_click || ctx.input(|i| i.key_pressed(Key::Escape)) {
            open = None;
        }
    }
    ctx.data_mut(|d| d.insert_temp(key, open));
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{vec2, Event, Modifiers, RawInput, Rect, TextEdit};

    fn frame_with(ctx: &Context, text: &mut String, events: Vec<Event>, select: bool) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
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
            show(ui.ctx());
        });
        // Nothing paints here; the font texture deltas must still be dropped explicitly.
        output.textures_delta.clear();
    }

    fn frame(ctx: &Context, text: &mut String, events: Vec<Event>) {
        frame_with(ctx, text, events, false);
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
}
