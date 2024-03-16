use nih_plug::prelude::*;
use nih_plug_egui::egui;
use std::sync::{Arc, RwLock};

use crate::{faust::widgets::*, DspType};

pub fn enum_combobox<T: strum::IntoEnumIterator + PartialEq + std::fmt::Debug>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    selected: &mut T,
) {
    egui::ComboBox::from_id_source(id)
        .selected_text(format!("{:?}", selected))
        .show_ui(ui, |ui| {
            for variant in T::iter() {
                let s = format!("{:?}", variant);
                ui.selectable_value(selected, variant, s);
            }
        });
}

pub fn top_panel_contents(
    async_executor: &AsyncExecutor<crate::NihFaustJit>,
    egui_ctx: &egui::Context,
    ui: &mut egui::Ui,
    dsp_nvoices_arc: &Arc<RwLock<i32>>,
    selected_paths_arc: &Arc<RwLock<crate::SelectedPaths>>,
    dsp_lib_path_dialog: &mut Option<egui_file::FileDialog>,
    dsp_script_dialog: &mut Option<egui_file::FileDialog>,
) {
    // Setting the DSP type and number of voices (if applicable):

    let mut nvoices = *dsp_nvoices_arc.read().unwrap();
    let mut selected_dsp_type = DspType::from_nvoices(nvoices);
    let last_dsp_type = selected_dsp_type;
    ui.horizontal(|ui| {
        ui.label("DSP type:");
        enum_combobox(ui, "dsp-type-combobox", &mut selected_dsp_type);
        match selected_dsp_type {
            DspType::AutoDetect => {
                nvoices = -1;
                ui.label("DSP type and number of voices will be detected from script metadata");
            }
            DspType::Effect => {
                nvoices = 0;
                ui.label("DSP will be loaded as monophonic effect");
            }
            DspType::Instrument => {
                if selected_dsp_type != last_dsp_type {
                    // If we just changed dsp_type to
                    // Instrument, we need to set a default
                    // voice number:
                    nvoices = 1;
                }
                ui.add(egui::Slider::new(&mut nvoices, 1..=32).text("voices"));
            }
        }
    });
    *dsp_nvoices_arc.write().unwrap() = nvoices;

    let mut selected_paths = selected_paths_arc.write().unwrap();

    // Setting the Faust libraries path:

    ui.label(format!(
        "Faust DSP libraries path: {}",
        selected_paths.dsp_lib_path.display()
    ));
    if (ui.button("Set Faust libraries path")).clicked() {
        let mut dialog =
            egui_file::FileDialog::select_folder(Some(selected_paths.dsp_lib_path.clone()));
        dialog.open();
        *dsp_lib_path_dialog = Some(dialog);
    }
    if let Some(dialog) = dsp_lib_path_dialog {
        if dialog.show(egui_ctx).selected() {
            if let Some(file) = dialog.path() {
                selected_paths.dsp_lib_path = file.to_path_buf();
            }
        }
    }

    // Setting the DSP script and triggering a reload:

    match &selected_paths.dsp_script {
        Some(path) => ui.label(format!("DSP script: {}", path.display())),
        None => ui.colored_label(egui::Color32::YELLOW, "No DSP script selected"),
    };
    if (ui.add(egui::Button::new("Set or reload DSP script").fill(egui::Color32::DARK_GRAY)))
        .clicked()
    {
        let presel = selected_paths
            .dsp_script
            .as_ref()
            .unwrap_or(&selected_paths.dsp_lib_path);
        let mut dialog = egui_file::FileDialog::open_file(Some(presel.clone()));
        dialog.open();
        *dsp_script_dialog = Some(dialog);
    }
    if let Some(dialog) = dsp_script_dialog {
        if dialog.show(egui_ctx).selected() {
            if let Some(file) = dialog.path() {
                selected_paths.dsp_script = Some(file.to_path_buf());
                async_executor.execute_background(crate::Tasks::ReloadDsp);
            }
        }
    }
}

pub fn central_panel_contents(ui: &mut egui::Ui, widgets: &mut [DspWidget<'_>]) {
    for w in widgets {
        match w {
            DspWidget::TabGroup {
                label,
                inner,
                selected,
            } => {
                egui::CollapsingHeader::new(&*label)
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("<<");
                                for (idx, w) in inner.iter().enumerate() {
                                    let mut btn = egui::Button::new(w.label());
                                    if *selected == idx {
                                        btn = btn.fill(egui::Color32::DARK_BLUE);
                                    }
                                    if ui.add(btn).clicked() {
                                        *selected = idx;
                                    }
                                }
                                ui.label(">>");
                            });
                            ui.separator();
                            central_panel_contents(ui, &mut inner[*selected..=*selected]);
                        });
                    });
            }
            DspWidget::Box {
                layout,
                label,
                inner,
            } => {
                let egui_layout = match layout {
                    BoxLayout::Horizontal => egui::Layout::left_to_right(egui::Align::Min),
                    BoxLayout::Vertical => egui::Layout::top_down(egui::Align::Min),
                };
                egui::CollapsingHeader::new(&*label)
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.with_layout(egui_layout, |ui| central_panel_contents(ui, inner))
                    });
            }
            DspWidget::Button {
                layout,
                label,
                zone,
            } => match layout {
                ButtonLayout::Trigger => {
                    // Not sure this is OK. The zone will be toggled off
                    // immediately the next time the GUI is drawn, so if the
                    // audio thread does not run in between it will miss the
                    // click
                    **zone = 0.0;
                    if ui.button(&*label).clicked() {
                        **zone = 1.0;
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
                ..
            } => {
                let rng = std::ops::RangeInclusive::new(*min, *max);
                match layout {
                    NumericLayout::NumEntry => {
                        ui.horizontal(|ui| {
                            ui.label(&*label);
                            ui.add(egui::DragValue::new(*zone).clamp_range(rng));
                        });
                    }
                    NumericLayout::HorizontalSlider => {
                        ui.vertical(|ui| {
                            ui.label(&*label);
                            ui.add(egui::Slider::new(*zone, rng).step_by(*step as f64));
                        });
                    }
                    NumericLayout::VerticalSlider => {
                        ui.vertical(|ui| {
                            ui.label(&*label);
                            ui.add(
                                egui::Slider::new(*zone, rng)
                                    .step_by(*step as f64)
                                    .vertical(),
                            );
                        });
                    }
                };
            }
            DspWidget::Bargraph {
                layout,
                label,
                zone,
                min,
                max,
            } => {
                // PLACEHOLDER:
                match layout {
                    BargraphLayout::Horizontal => ui.vertical(|ui| {
                        ui.label(format!("{}:", label));
                        ui.horizontal(|ui| {
                            ui.label(format!("{:.2}[", min));
                            ui.colored_label(egui::Color32::YELLOW, format!("{:.2}", **zone));
                            ui.label(format!("]{:.2}", max));
                        });
                    }),
                    BargraphLayout::Vertical => ui.vertical(|ui| {
                        ui.label(format!("{}:", label));
                        ui.label(format!("_{:.2}_", max));
                        ui.colored_label(egui::Color32::YELLOW, format!(" {:.2}", **zone));
                        ui.label(format!("¨{:.2}¨", min));
                    }),
                };
            }
        }
    }
}
