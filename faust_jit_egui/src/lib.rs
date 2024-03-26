use egui::{Align, Layout, Sense};
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

trait DoIf: Sized {
    /// Call a function on self if a condition is met
    fn do_if(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self;

    /// Call a function on self if an option value is Some
    fn do_if_some<T>(self, cond: Option<T>, f: impl FnOnce(Self, T) -> Self) -> Self;
}

impl<T> DoIf for T {
    fn do_if(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if cond {
            f(self)
        } else {
            self
        }
    }

    fn do_if_some<X>(self, option: Option<X>, f: impl FnOnce(Self, X) -> Self) -> Self {
        match option {
            Some(x) => f(self, x),
            None => self,
        }
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
                        let btn = egui::Button::new(w.label())
                            .do_if(*selected == idx, |s| s.fill(egui::Color32::DARK_BLUE));
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
                    BoxLayout::Horizontal => Layout::left_to_right(Align::Min),
                    BoxLayout::Vertical => Layout::top_down(Align::Min),
                    _ => panic!("Cannot be Tab here"),
                };
                let mut draw_inner = |ui: &mut egui::Ui| {
                    ui.with_layout(egui_layout, |ui| faust_widgets_ui_rec(ui, inner, false))
                };
                if in_a_tab || label.is_empty() {
                    draw_inner(ui);
                } else {
                    egui::CollapsingHeader::new(&*label)
                        .default_open(true)
                        .do_if(*layout == BoxLayout::Horizontal, |s| {
                            s.icon(hgroup_header_icon)
                        })
                        .show(ui, draw_inner);
                }
            }
            DspWidget::BoolParam {
                layout,
                label,
                zone,
                hidden: false,
                tooltip,
            } => {
                let resp = match layout {
                    BoolParamLayout::Held => {
                        let button = egui::Button::new(&*label)
                            .sense(Sense::drag().union(Sense::hover()))
                            .do_if(**zone != 0.0, |s| {
                                // If the gate is currently on:
                                s.fill(egui::Color32::from_rgb(115, 115, 50))
                            });
                        let resp = ui.add(button);
                        if resp.drag_started() {
                            // If the button just started to be held:
                            **zone = 1.0;
                        } else if resp.drag_released() {
                            // If the button was just released:
                            **zone = 0.0;
                        }
                        resp
                    }
                    BoolParamLayout::Checkbox => {
                        let mut selected = **zone != 0.0;
                        let resp = ui.checkbox(&mut selected, &*label).interact(Sense::hover());
                        **zone = selected as i32 as f32;
                        resp
                    }
                };
                if let Some(txt) = tooltip {
                    resp.on_hover_text(txt.to_owned());
                }
            }
            DspWidget::NumParam {
                layout,
                style: _,
                label,
                zone,
                min,
                max,
                step,
                init,
                metadata:
                    NumMetadata {
                        unit,
                        scale,
                        hidden: false,
                        tooltip,
                    },
            } => {
                let rng = std::ops::RangeInclusive::new(*min, *max);
                ui.vertical(|ui| {
                    if !label.is_empty() {
                        let resp = ui
                            .label(&*label)
                            .interact(Sense::click().union(Sense::hover()))
                            .do_if_some(tooltip.as_deref(), |s, tooltip| {
                                s.on_hover_text(tooltip.to_owned())
                            });
                        if resp.double_clicked() {
                            **zone = *init;
                        }
                    }
                    match layout {
                        NumParamLayout::NumEntry => ui.add(
                            egui::DragValue::new(*zone)
                                .clamp_range(rng)
                                .do_if_some(unit.as_deref(), |s, unit| s.suffix(unit)),
                        ),
                        _ => ui.add(
                            egui::Slider::new(*zone, rng)
                                .step_by(*step as f64)
                                .do_if_some(unit.as_deref(), |s, unit| s.suffix(unit))
                                .do_if(*layout == NumParamLayout::VerticalSlider, |s| s.vertical())
                                .do_if(*scale == WidgetScale::Log, |s| s.logarithmic(true)), // TODO: Deal with Exp
                        ),
                    };
                });
            }
            DspWidget::NumDisplay {
                layout,
                style: _,
                label,
                zone,
                min,
                max,
                metadata:
                    NumMetadata {
                        unit,
                        scale: _,
                        hidden: false,
                        tooltip,
                    },
            } => {
                let cur_val = **zone;
                let t = (cur_val - *min) / (*max - *min);
                let unit_or_empty = unit.as_deref().unwrap_or("");

                ui.vertical(|ui| {
                    let label_width = if !label.is_empty() {
                        let resp = ui
                            .label(&*label)
                            .interact(Sense::hover())
                            .do_if_some(tooltip.as_deref(), |s, tooltip| {
                                s.on_hover_text(tooltip.to_owned())
                            });
                        resp.rect.max.x - resp.rect.min.x
                    } else {
                        30.0
                    };
                    match layout {
                        NumDisplayLayout::Horizontal => {
                            ui.horizontal(|ui| {
                                ui.label(format!("{:.2}", min));
                                draw_bargraph(ui, t, cur_val, unit_or_empty, layout);
                                ui.label(format!("{:.2}{}", max, unit_or_empty));
                            });
                        }
                        NumDisplayLayout::Vertical => {
                            ui.allocate_ui_with_layout(
                                egui::vec2(label_width, 100.0),
                                Layout::top_down(Align::Center),
                                |ui| {
                                    ui.label(format!("{:.2}{}", max, unit_or_empty));
                                    draw_bargraph(ui, t, cur_val, unit_or_empty, layout);
                                    ui.label(format!("{:.2}", min));
                                },
                            );
                        }
                    };
                });
            }
            _ => {}
        }
    }
}

fn draw_bargraph(
    ui: &mut egui::Ui,
    mut t: f32,
    cur_val: f32,
    unit: &str,
    layout: &NumDisplayLayout,
) {
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
        NumDisplayLayout::Horizontal => egui::vec2(80.0, 20.0),
        NumDisplayLayout::Vertical => egui::vec2(20.0, 80.0),
    };

    let (rsp, painter) = ui.allocate_painter(max_size, Sense::hover());

    let (right_pos, top_pos) = match layout {
        NumDisplayLayout::Horizontal => (
            (1.0 - t) * rsp.rect.min.x + t * rsp.rect.max.x,
            rsp.rect.min.y,
        ),
        NumDisplayLayout::Vertical => (
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

    rsp.on_hover_text(format!("{} {}", cur_val, unit));
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
