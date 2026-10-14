//! Desktop adapter for the KML hierarchy; mutations remain in core.
use crate::app::App;
use earthcraft_core::{
    language::Lang,
    placemark::{PlaceItem, PlaceNode, Placemark},
};
use eframe::egui::{self, collapsing_header::CollapsingState};

const TREE_HEIGHT_POINTS: f32 = 128.0;
const DESCRIPTION_ROWS: usize = 3;
const TREE_INDENT_POINTS: f32 = 8.0;
const TREE_ICON_SIZE_POINTS: f32 = 12.0;
const TREE_ITEM_GAP_POINTS: f32 = 2.0;

fn tree_scroll_area() -> egui::ScrollArea {
    egui::ScrollArea::vertical()
        .id_salt("places-tree")
        .max_height(TREE_HEIGHT_POINTS)
        .min_scrolled_height(TREE_HEIGHT_POINTS)
        .auto_shrink([false, false])
}

fn compact_tree_style(ui: &mut egui::Ui) {
    let row_height = ui.text_style_height(&egui::TextStyle::Body) + 2.0;
    let spacing = ui.spacing_mut();
    spacing.indent = TREE_INDENT_POINTS;
    spacing.item_spacing = egui::vec2(TREE_ITEM_GAP_POINTS, 0.0);
    spacing.button_padding = egui::vec2(2.0, 0.0);
    spacing.interact_size.y = row_height;
    spacing.icon_width = TREE_ICON_SIZE_POINTS;
    spacing.icon_width_inner = TREE_ICON_SIZE_POINTS - 4.0;
    spacing.icon_spacing = TREE_ITEM_GAP_POINTS;
}

fn paint_icon(ui: &egui::Ui, rect: egui::Rect, folder: bool, stroke: egui::Stroke) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let rect = rect.shrink(1.0);
    if folder {
        let at = |x, y| rect.min + rect.size() * egui::vec2(x, y);
        ui.painter().add(egui::Shape::closed_line(
            vec![
                at(0.0, 0.2),
                at(0.0, 0.0),
                at(0.4, 0.0),
                at(0.55, 0.2),
                at(1.0, 0.2),
                at(1.0, 1.0),
                at(0.0, 1.0),
            ],
            stroke,
        ));
    } else {
        let center = egui::pos2(rect.center().x, rect.top() + rect.height() * 0.3);
        let radius = rect.width() * 0.3;
        ui.painter().circle_stroke(center, radius, stroke);
        ui.painter().line_segment(
            [
                center + egui::vec2(0.0, radius),
                egui::pos2(center.x, rect.bottom()),
            ],
            stroke,
        );
    }
}
enum Action {
    Delete(u64),
    Move(u64, Option<u64>),
}

fn description_editor(
    ui: &mut egui::Ui,
    id: u64,
    description: &mut String,
    lang: Lang,
) -> egui::scroll_area::ScrollAreaOutput<egui::Response> {
    let height = ui.text_style_height(&egui::TextStyle::Body) * DESCRIPTION_ROWS as f32
        + 2.0 * ui.spacing().button_padding.y;
    egui::ScrollArea::vertical()
        .id_salt(("place-description", id))
        .max_height(height)
        .min_scrolled_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(description)
                    .hint_text(lang.text("places-description-placeholder"))
                    .desired_rows(DESCRIPTION_ROWS)
                    .desired_width(f32::INFINITY),
            )
        })
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
    let icon_id = egui::Id::new("place-icon").with(id);
    let button = egui::Button::selectable(
        *selected == Some(id),
        (
            egui::Atom::custom(icon_id, egui::Vec2::splat(TREE_ICON_SIZE_POINTS)),
            node.name(),
            egui::Atom::grow(),
        ),
    )
    .truncate()
    .min_size(egui::vec2(
        ui.available_width(),
        ui.spacing().interact_size.y,
    ))
    .sense(egui::Sense::click_and_drag())
    .atom_ui(ui);
    if let Some(rect) = button.rect(icon_id) {
        paint_icon(
            ui,
            rect,
            folder,
            ui.style()
                .interact_selectable(&button.response, *selected == Some(id))
                .fg_stroke,
        );
    }
    let response = button.response.on_hover_text(node.name());
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
                let gap = ui.spacing().item_spacing.x;
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(ui.spacing().indent);
                ui.spacing_mut().item_spacing.x = gap;
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
            compact_tree_style(ui);
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
            tree_scroll_area().show(ui, |ui| {
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
            description_editor(ui, id, node.description_mut(), lang);
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
mod tests;
