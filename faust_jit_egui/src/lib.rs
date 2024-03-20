use faust_jit::widgets::*;

fn hgroup_header_icon(ui: &mut egui::Ui, openness: f32, response: &egui::Response) {
    let stroke = ui.style().interact(&response).fg_stroke;
    let radius = 3.0;
    if openness == 0.0 {
        ui.painter()
            .circle_filled(response.rect.center(), radius, stroke.color);
    } else {
        ui.painter()
            .circle_stroke(response.rect.center(), radius, stroke);
    }
}

fn faust_widgets_ui_rec(ui: &mut egui::Ui, widgets: &mut [DspWidget<&mut f32>], in_a_tab: bool) {
    for w in widgets {
        match w {
            DspWidget::Box {
                layout: BoxLayout::Tab { selected },
                label,
                inner,
            } => {
                let id = ui.make_persistent_id(&label);
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    id,
                    true,
                )
                .show_header(ui, |ui| {
                    ui.label(&*label);
                    for (idx, w) in inner.iter().enumerate() {
                        let mut btn = egui::Button::new(w.label());
                        if *selected == idx {
                            btn = btn.fill(egui::Color32::DARK_BLUE);
                        }
                        if ui.add(btn).clicked() {
                            *selected = idx;
                        }
                    }
                })
                .body(|ui| {
                    faust_widgets_ui_rec(ui, &mut inner[*selected..=*selected], true);
                });
            }
            DspWidget::Box {
                layout, // Not a Tab
                label,
                inner,
            } => {
                let egui_layout = match layout {
                    BoxLayout::Horizontal => egui::Layout::left_to_right(egui::Align::Min),
                    BoxLayout::Vertical => egui::Layout::top_down(egui::Align::Min),
                    _ => panic!("Cannot be Tab here"),
                };
                let mut draw_inner = |ui: &mut egui::Ui| {
                    ui.with_layout(egui_layout, |ui| faust_widgets_ui_rec(ui, inner, false))
                };
                if in_a_tab || label.is_empty() {
                    draw_inner(ui);
                } else {
                    let mut header = egui::CollapsingHeader::new(&*label).default_open(true);
                    if *layout == BoxLayout::Horizontal {
                        header = header.icon(hgroup_header_icon);
                    }
                    header.show(ui, draw_inner);
                }
            }
            DspWidget::Button {
                layout,
                label,
                zone,
            } => match layout {
                ButtonLayout::Held => {
                    let mut button = egui::Button::new(&*label).sense(egui::Sense::drag());
                    if **zone != 0.0 {
                        // If the gate is currently on:
                        button = button.fill(egui::Color32::from_rgb(115, 115, 50));
                    }
                    let rsp = ui.add(button);
                    if rsp.drag_started() {
                        // If the button just started to be held:
                        **zone = 1.0;
                    } else if rsp.drag_released() {
                        // If the button was just released:
                        **zone = 0.0;
                    }
                }
                ButtonLayout::Checkbox => {
                    let mut selected = **zone != 0.0;
                    ui.checkbox(&mut selected, &*label);
                    **zone = selected as i32 as f32;
                }
            },
            DspWidget::Numeric {
                layout,
                label,
                zone,
                min,
                max,
                step,
                init,
            } => {
                let rng = std::ops::RangeInclusive::new(*min, *max);
                ui.vertical(|ui| {
                    if !label.is_empty() {
                        if ui
                            .label(&*label)
                            .interact(egui::Sense::click())
                            .double_clicked()
                        {
                            **zone = *init;
                        }
                    }
                    match layout {
                        NumericLayout::NumEntry => {
                            ui.add(egui::DragValue::new(*zone).clamp_range(rng))
                        }
                        NumericLayout::HorizontalSlider => {
                            ui.add(egui::Slider::new(*zone, rng).step_by(*step as f64))
                        }
                        NumericLayout::VerticalSlider => ui.add(
                            egui::Slider::new(*zone, rng)
                                .step_by(*step as f64)
                                .vertical(),
                        ),
                    };
                });
            }
            DspWidget::Bargraph {
                layout,
                label,
                zone,
                min,
                max,
            } => {
                let cur_val = **zone;
                let t = (cur_val - *min) / (*max - *min);

                ui.vertical(|ui| {
                    let label_width = if !label.is_empty() {
                        let resp = ui.label(&*label);
                        resp.rect.max.x - resp.rect.min.x
                    } else {
                        30.0
                    };
                    match layout {
                        BargraphLayout::Horizontal => {
                            ui.horizontal(|ui| {
                                ui.label(format!("{:.2}", min));
                                draw_bargraph(ui, t, cur_val, layout);
                                ui.label(format!("{:.2}", max));
                            });
                        }
                        BargraphLayout::Vertical => {
                            ui.allocate_ui_with_layout(
                                egui::vec2(label_width, 100.0),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.label(format!("{:.2}", max));
                                    draw_bargraph(ui, t, cur_val, layout);
                                    ui.label(format!("{:.2}", min));
                                },
                            );
                        }
                    };
                });
            }
        }
    }
}

fn draw_bargraph(ui: &mut egui::Ui, mut t: f32, cur_val: f32, layout: &BargraphLayout) {
    let min_color = egui::Color32::DARK_GREEN;
    let max_color = egui::Color32::YELLOW;

    let cur_color = if t < 0.0 {
        t = 0.0;
        min_color
    } else if t > 1.0 {
        t = 1.0;
        egui::Color32::RED
    } else {
        lerp_colors(min_color, max_color, t)
    };

    let max_size = match layout {
        BargraphLayout::Horizontal => egui::vec2(80.0, 20.0),
        BargraphLayout::Vertical => egui::vec2(20.0, 80.0),
    };

    let (rsp, painter) = ui.allocate_painter(max_size, egui::Sense::hover());

    let (right_pos, top_pos) = match layout {
        BargraphLayout::Horizontal => (
            (1.0 - t) * rsp.rect.min.x + t * rsp.rect.max.x,
            rsp.rect.min.y,
        ),
        BargraphLayout::Vertical => (
            rsp.rect.max.x,
            (1.0 - t) * rsp.rect.max.y + t * rsp.rect.min.y,
        ),
    };

    let rounding = egui::Rounding::same(3.0);
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rsp.rect.min.x, top_pos),
            egui::pos2(right_pos, rsp.rect.max.y),
        ),
        rounding,
        cur_color,
    );
    painter.rect_stroke(
        rsp.rect,
        rounding,
        egui::Stroke {
            width: 2.0,
            color: ui.style().interact(&rsp).fg_stroke.color,
        },
    );

    rsp.on_hover_text(format!("{}", cur_val));
}

fn lerp_colors(min: egui::Color32, max: egui::Color32, t: f32) -> egui::Color32 {
    let lerp_comps = |x, y| ((1.0 - t) * x as f32 + t * y as f32) as u8;
    egui::Color32::from_rgb(
        lerp_comps(min.r(), max.r()),
        lerp_comps(min.g(), max.g()),
        lerp_comps(min.b(), max.b()),
    )
}

/// Draw and update the faust widgets inside an egui::Ui
pub fn faust_widgets_ui(ui: &mut egui::Ui, widgets: &mut [DspWidget<&mut f32>]) {
    faust_widgets_ui_rec(ui, widgets, false);
}
