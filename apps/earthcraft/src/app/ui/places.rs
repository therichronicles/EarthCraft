//! Desktop adapter for the KML hierarchy; mutations remain in core.
use crate::app::App;
use earthcraft_core::{
    language::Lang,
    placemark::{PlaceItem, PlaceNode, Placemark},
};
use eframe::egui::{self, collapsing_header::CollapsingState};

const TREE_MAX_HEIGHT_POINTS: f32 = 320.0;
enum Action {
    Delete(u64),
    Move(u64, Option<u64>),
}

fn row(
    ui: &mut egui::Ui,
    node: &mut PlaceNode,
    selected: &mut Option<u64>,
    action: &mut Option<Action>,
    goto: &mut Option<(f64, f64, f64)>,
    lang: Lang,
) -> egui::Response {
    let id = node.id();
    let folder = matches!(node.item, PlaceItem::Folder { .. });
    ui.checkbox(&mut node.visible, "")
        .on_hover_text(lang.text("places-visible-help"));
    let icon = if folder { "▰" } else { "●" };
    let response = ui.add_sized(
        [ui.available_width(), ui.spacing().interact_size.y],
        egui::Button::selectable(
            *selected == Some(id),
            (format!("{icon}  {}", node.name()), egui::Atom::grow()),
        )
        .truncate()
        .sense(egui::Sense::click_and_drag()),
    );
    if response.clicked() || response.secondary_clicked() {
        *selected = Some(id);
    }
    if response.double_clicked()
        && let PlaceItem::Marker(pm) = &node.item
    {
        *goto = Some((pm.lat, pm.lon, 3.0));
    }
    response.dnd_set_drag_payload(id);
    if folder && let Some(source) = response.dnd_release_payload::<u64>() {
        *action = Some(Action::Move(*source, Some(id)));
    }
    response.context_menu(|ui| {
        if let PlaceItem::Marker(pm) = &node.item
            && ui.button(lang.text("placemarks-fly-to-action")).clicked()
        {
            *goto = Some((pm.lat, pm.lon, 3.0));
            ui.close();
        }
        if ui.button(lang.text("places-delete-action")).clicked() {
            *action = Some(Action::Delete(id));
            ui.close();
        }
    });
    response
}

fn nodes(
    ui: &mut egui::Ui,
    entries: &mut [PlaceNode],
    selected: &mut Option<u64>,
    action: &mut Option<Action>,
    goto: &mut Option<(f64, f64, f64)>,
    lang: Lang,
) {
    for node in entries {
        if matches!(node.item, PlaceItem::Folder { .. }) {
            CollapsingState::load_with_default_open(
                ui.ctx(),
                egui::Id::new("place-node").with(node.id()),
                true,
            )
            .show_header(ui, |ui| row(ui, node, selected, action, goto, lang))
            .body(|ui| {
                if let PlaceItem::Folder { children, .. } = &mut node.item {
                    nodes(ui, children, selected, action, goto, lang);
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.add_space(ui.spacing().indent);
                row(ui, node, selected, action, goto, lang);
            });
        }
    }
}

impl App {
    pub(in crate::app) fn places_tree(&mut self, ui: &mut egui::Ui) -> Option<(f64, f64, f64)> {
        let lang = self.lang;
        let mut goto = None;
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_name)
                    .hint_text(lang.text("placemarks-name-placeholder"))
                    .desired_width(110.0),
            );
            let parent = self.place_selection.filter(|id| {
                self.placemarks
                    .find_mut(*id)
                    .is_some_and(|node| matches!(node.item, PlaceItem::Folder { .. }))
            });
            if ui
                .button(lang.text("placemarks-add-center-action"))
                .clicked()
            {
                let name = if self.new_name.trim().is_empty() {
                    lang.format(
                        "placemarks-generated-name",
                        &[("number", (self.placemarks.len() + 1).to_string().into())],
                    )
                } else {
                    self.new_name.trim().to_owned()
                };
                let node = PlaceNode::marker(Placemark {
                    name,
                    desc: String::new(),
                    lat: self.cam.lat,
                    lon: self.cam.lon,
                });
                self.place_selection = Some(node.id());
                self.placemarks.insert(parent, node);
                self.new_name.clear();
            }
            if ui.button(lang.text("places-add-folder-action")).clicked() {
                let name = if self.new_name.trim().is_empty() {
                    lang.text("places-folder-name").into()
                } else {
                    self.new_name.trim().to_owned()
                };
                let node = PlaceNode::folder(name, String::new(), false, Vec::new());
                self.place_selection = Some(node.id());
                self.placemarks.insert(parent, node);
                self.new_name.clear();
            }
        });
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let root = ui.add_sized(
                [ui.available_width(), ui.spacing().interact_size.y],
                egui::Button::selectable(
                    self.place_selection.is_none(),
                    (lang.text("places-root-name"), egui::Atom::grow()),
                )
                .truncate(),
            );
            if root.clicked() {
                self.place_selection = None;
            }
            if let Some(source) = root.dnd_release_payload::<u64>() {
                action = Some(Action::Move(*source, None));
            }
            egui::ScrollArea::vertical()
                .id_salt("places-tree")
                .max_height(TREE_MAX_HEIGHT_POINTS)
                .show(ui, |ui| {
                    nodes(
                        ui,
                        &mut self.placemarks.roots,
                        &mut self.place_selection,
                        &mut action,
                        &mut goto,
                        lang,
                    );
                    if self.placemarks.is_empty() {
                        ui.weak(lang.text("places-empty-help"));
                    }
                });
        });
        if let Some(id) = self.place_selection
            && let Some(node) = self.placemarks.find_mut(id)
        {
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(node.name_mut()).desired_width(f32::INFINITY));
            ui.add(
                egui::TextEdit::multiline(node.description_mut())
                    .hint_text(lang.text("places-description-placeholder"))
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                if let PlaceItem::Marker(pm) = &node.item
                    && ui.button(lang.text("placemarks-fly-to-action")).clicked()
                {
                    goto = Some((pm.lat, pm.lon, 3.0));
                }
                if ui.button(lang.text("places-delete-action")).clicked() {
                    action = Some(Action::Delete(id));
                }
            });
        }
        match action {
            Some(Action::Delete(id)) => {
                self.placemarks.remove(id);
                self.place_selection = None;
            }
            Some(Action::Move(id, parent)) => {
                self.placemarks.move_into(id, parent);
            }
            None => {}
        }
        goto
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn row_selection_accepts_clicks_past_the_name() {
        let ctx = egui::Context::default();
        let mut node = PlaceNode::marker(Placemark {
            name: "Peak".into(),
            desc: String::new(),
            lat: 27.0,
            lon: 86.0,
        });
        let mut selection = None;
        let mut action = None;
        let mut goto = None;
        let mut bounds = egui::Rect::NOTHING;
        for frame in 0..4 {
            let point = egui::pos2(bounds.max.x - 3.0, bounds.center().y);
            let events = match frame {
                2 | 3 => vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 2,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                _ => Vec::new(),
            };
            crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320.0, 240.0),
                    )),
                    events,
                    time: Some(f64::from(frame) * 0.1),
                    ..Default::default()
                },
                |ui| {
                    ui.horizontal(|ui| {
                        bounds = row(
                            ui,
                            &mut node,
                            &mut selection,
                            &mut action,
                            &mut goto,
                            Lang::En,
                        )
                        .rect;
                    });
                },
            );
        }
        assert!(bounds.width() > 250.0, "{bounds:?}");
        assert_eq!(selection, Some(node.id()));
        assert!(node.visible);
        assert!(goto.is_none());
    }
}
