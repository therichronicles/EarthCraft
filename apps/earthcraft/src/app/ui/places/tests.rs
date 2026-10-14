use super::*;

#[test]
fn tree_reserves_its_height_even_near_the_bottom_of_the_sidebar() {
    let ctx = egui::Context::default();
    for rows in [1, 100] {
        for _ in 0..3 {
            crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(270.0, 500.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add_space(400.0);
                        let area = tree_scroll_area().show(ui, |ui| {
                            for row in 0..rows {
                                ui.label(format!("Place {row}"));
                            }
                        });
                        assert!((area.inner_rect.height() - TREE_HEIGHT_POINTS).abs() < 1.0);
                        if rows == 100 {
                            assert!(area.content_size.y > area.inner_rect.height());
                        }
                    });
                },
            );
        }
    }
}

#[test]
fn long_descriptions_stay_small_and_scroll_without_changing_the_text() {
    let ctx = egui::Context::default();
    let original = "<!DOCTYPE html>\n<html><body><script>window.location.href='https://example.com';</script></body></html>\n".repeat(100);
    let mut description = original.clone();
    let mut scrolled = false;
    for frame in 0..8 {
        crate::app::test_ui::run(
            &ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(270.0, 500.0),
                )),
                time: Some(frame as f64 * 0.1),
                events: if frame == 2 {
                    vec![
                        egui::Event::PointerMoved(egui::pos2(100.0, 20.0)),
                        egui::Event::MouseWheel {
                            phase: egui::TouchPhase::Move,
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -100.0),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                let height = ui.text_style_height(&egui::TextStyle::Body) * DESCRIPTION_ROWS as f32
                    + 2.0 * ui.spacing().button_padding.y;
                let area = description_editor(ui, 1, &mut description, Lang::En);
                assert!(area.inner_rect.height() <= height + 1.0);
                assert!(area.content_size.y > area.inner_rect.height() * 10.0);
                scrolled |= area.state.offset.y > 0.0;
            },
        );
    }
    assert!(scrolled);
    assert_eq!(description, original);
}
#[test]
fn nested_tree_fits_default_sidebar_with_compact_rows_and_vector_icons() {
    let ctx = egui::Context::default();
    let mut entries = vec![
        PlaceNode::marker(Placemark {
            name: "Los Angeles".into(),
            desc: String::new(),
            lat: 34.0,
            lon: -118.0,
        }),
        PlaceNode::marker(Placemark {
            name: "Like's Thai and Chinese Food Takeout".into(),
            desc: String::new(),
            lat: 34.0,
            lon: -118.0,
        }),
    ];
    for name in [
        "Los Angeles",
        "California",
        "United States",
        "Northern America",
        "My Places",
        "My Places 3.kml",
    ] {
        entries = vec![PlaceNode::folder(
            name.into(),
            String::new(),
            false,
            entries,
        )];
    }
    for width in [270.0, 210.0, 270.0] {
        let mut bounds = egui::Rect::NOTHING;
        let output = crate::app::test_ui::run(
            &ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 400.0),
                )),
                ..Default::default()
            },
            |ui| {
                ui.indent("placemarks-section", |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        compact_tree_style(ui);
                        let available = ui.available_rect_before_wrap();
                        nodes(ui, &mut entries, &mut None, &mut None, &mut None, Lang::En);
                        bounds = ui.min_rect();
                        assert!(
                            bounds.right() <= available.right() + 0.1,
                            "{bounds:?} vs {available:?}"
                        );
                    });
                });
                assert!(
                    bounds.height()
                        <= 8.0 * (ui.text_style_height(&egui::TextStyle::Body) + 2.0) + 1.0,
                    "{bounds:?}"
                );
            },
        );
        let mut folder_icon = false;
        let mut point_icon = false;
        let mut los_angeles_labels = 0;
        for shape in &output.shapes {
            match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Los Angeles" => {
                    assert!(!text.galley.elided);
                    assert!(text.pos.x < 145.0, "deep leaf starts at {}", text.pos.x);
                    los_angeles_labels += 1;
                }
                egui::Shape::Path(path) if path.closed && path.points.len() == 7 => {
                    folder_icon = true
                }
                egui::Shape::Circle(_) => point_icon = true,
                _ => {}
            }
        }
        assert_eq!(los_angeles_labels, 2);
        assert!(folder_icon && point_icon);
    }
}
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
