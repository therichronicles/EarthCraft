//! Sidebar shell: navigation and layout stay independent of scene/tool state.
use super::super::App;
use earthcraft_core::language::Lang;
use earthcraft_data::settings::sidebar::{
    MAX_SIDEBAR_WIDTH_POINTS, MIN_PINNED_PLACES_HEIGHT_POINTS, MIN_SIDEBAR_WIDTH_POINTS,
    SidebarLayout, SidebarPanel,
};
use eframe::egui;
mod explore;
mod layers;
mod places;
mod rail;

const SIDEBAR_TITLE_SCALE: f32 = 1.1;
const PINNED_PLACES_MAX_HEIGHT_FRACTION: f32 = 0.65;

impl App {
    pub(in crate::app) fn side_panel(&mut self, ui: &mut egui::Ui) {
        let mut layout = self.settings.sidebar.clone();
        route_shortcuts(
            &mut layout,
            self.place_reveal_requested,
            self.ruler_reveal_requested,
        );
        let mut open = self.sidebar_open;
        self.sidebar_animating =
            show_shell(
                ui,
                &mut layout,
                &mut open,
                self.lang,
                |ui, panel| match panel {
                    SidebarPanel::Explore => self.sidebar_explore(ui),
                    SidebarPanel::Layers => self.sidebar_layers(ui),
                    SidebarPanel::Places => self.sidebar_places(ui),
                },
            );
        self.sidebar_open = open;
        self.settings.sidebar = layout;
    }
}

fn show_shell(
    ui: &mut egui::Ui,
    layout: &mut SidebarLayout,
    open: &mut bool,
    lang: Lang,
    mut draw_panel: impl FnMut(&mut egui::Ui, SidebarPanel),
) -> bool {
    let openness = ui.animate_bool_responsive(egui::Id::new("side").with("animation"), *open);
    let animating = openness != if *open { 1.0 } else { 0.0 };
    let mut expanded = *open;
    let response = egui::Panel::show_switched(
        ui,
        &mut expanded,
        egui::Panel::left("side_collapsed")
            .frame(egui::Frame::NONE)
            .exact_size(0.0),
        expanded_sidebar_panel(layout.width_points),
        |ui, _expanded| {
            if animating || !*open {
                return;
            }
            navigation_header(ui, layout, lang);
            let panel = layout.panel;
            ui.separator();
            if layout.places_pinned && panel != SidebarPanel::Places {
                let max_height = (ui.available_height() * PINNED_PLACES_MAX_HEIGHT_FRACTION)
                    .max(MIN_PINNED_PLACES_HEIGHT_POINTS);
                let pinned = egui::Panel::bottom("pinned-places")
                    .default_size(layout.pinned_places_height_points)
                    .size_range(MIN_PINNED_PLACES_HEIGHT_POINTS..=max_height)
                    .resizable(true)
                    .show(ui, |ui| {
                        places_header(ui, layout, lang);
                        draw_panel(ui, SidebarPanel::Places);
                    });
                if !ui.input(|i| i.pointer.any_down()) {
                    layout.pinned_places_height_points = pinned.response.rect.height();
                }
            }
            if panel == SidebarPanel::Places {
                draw_panel(ui, panel);
                return;
            }
            sidebar_scroll_area(match panel {
                SidebarPanel::Explore => "explore-scroll",
                SidebarPanel::Layers => "layers-scroll",
                SidebarPanel::Places => "places-scroll",
            })
            .show(ui, |ui| draw_panel(ui, panel));
        },
    );
    if *open && !animating && !ui.input(|i| i.pointer.any_down()) {
        layout.width_points = response.response.rect.width();
    }
    *open = expanded;
    layout.open = *open;
    animating
}

/// Align the expanded sidebar; retain a usable coordinate slot when it is closed.
pub(super) fn outer_right(ui: &egui::Ui, layout: &SidebarLayout, open: bool) -> f32 {
    let left = ui.content_rect().left();
    if open {
        egui::containers::panel::PanelState::load(ui, egui::Id::new("side"))
            .map_or(left + layout.width_points, |state| state.outer_rect.right())
    } else {
        left + layout.width_points
    }
}

fn panel_title(panel: SidebarPanel, lang: Lang) -> &'static str {
    lang.text(match panel {
        SidebarPanel::Explore => "sidebar-explore-title",
        SidebarPanel::Layers => "layers-section-title",
        SidebarPanel::Places => "sidebar-places-title",
    })
}

fn route_shortcuts(layout: &mut SidebarLayout, place: bool, ruler: bool) {
    if ruler {
        layout.panel = SidebarPanel::Explore;
    } else if place && !layout.places_pinned {
        layout.panel = SidebarPanel::Places;
    }
}

fn navigation_header(ui: &mut egui::Ui, layout: &mut SidebarLayout, lang: Lang) {
    ui.horizontal(|ui| {
        rail::show(ui, layout, lang);
        ui.separator();
        panel_heading(ui, layout.panel, lang);
        if layout.panel == SidebarPanel::Places {
            pin_button(ui, layout, lang);
        }
    });
}

fn places_header(ui: &mut egui::Ui, layout: &mut SidebarLayout, lang: Lang) {
    ui.horizontal(|ui| {
        panel_heading(ui, SidebarPanel::Places, lang);
        pin_button(ui, layout, lang);
    });
}

fn panel_heading(ui: &mut egui::Ui, panel: SidebarPanel, lang: Lang) {
    let size = ui.style().text_styles[&egui::TextStyle::Body].size * SIDEBAR_TITLE_SCALE;
    ui.label(
        egui::RichText::new(panel_title(panel, lang))
            .size(size)
            .strong(),
    );
}

fn pin_button(ui: &mut egui::Ui, layout: &mut SidebarLayout, lang: Lang) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.toggle_value(
            &mut layout.places_pinned,
            lang.text("sidebar-pin-places-action"),
        )
        .on_hover_text(lang.text(if layout.places_pinned {
            "sidebar-unpin-places-tooltip"
        } else {
            "sidebar-pin-places-tooltip"
        }));
    });
}

fn expanded_sidebar_panel(width_points: f32) -> egui::Panel {
    egui::Panel::left("side")
        .default_size(width_points)
        .size_range(MIN_SIDEBAR_WIDTH_POINTS..=MAX_SIDEBAR_WIDTH_POINTS)
        .resizable(true)
}
fn sidebar_scroll_area(id: &str) -> egui::ScrollArea {
    egui::ScrollArea::both()
        .id_salt(id)
        .auto_shrink([false, false])
}

#[cfg(test)]
mod tests;
