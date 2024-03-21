use nih_plug::prelude::*;
use nih_plug_egui::egui;
use std::sync::{Arc, RwLock};

use crate::{DspLoadingMode, DspState};

/// Data shared between the plugin and the GUI thread
pub(crate) struct EditorArcs {
    pub(crate) nih_egui_state: Arc<nih_plug_egui::EguiState>,
    pub(crate) selected_paths: Arc<RwLock<crate::SelectedPaths>>,
    pub(crate) dsp_state: Arc<RwLock<DspState>>,
    pub(crate) dsp_nvoices: Arc<RwLock<i32>>,
}

/// Data owned only by the GUI thread
struct EditorState {
    script_dialog: Option<egui_file::FileDialog>,
    lib_path_dialog: Option<egui_file::FileDialog>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            script_dialog: None,
            lib_path_dialog: None,
        }
    }
}

pub(crate) fn create_editor(
    arcs: EditorArcs,
    async_executor: AsyncExecutor<crate::NihFaustJit>,
) -> Option<Box<dyn Editor>> {
    nih_plug_egui::create_egui_editor(
        Arc::clone(&arcs.nih_egui_state),
        EditorState::default(),
        |_, _| {},
        move |egui_ctx, _param_setter, ed_state| {
            if arcs.nih_egui_state.is_open() {
                // Top panel (DSP settings, loading):
                egui::TopBottomPanel::top("DSP loading")
                    .frame(egui::Frame::default().inner_margin(8.0))
                    .show(egui_ctx, |ui| {
                        top_panel_contents(ui, &arcs, &async_executor, ed_state);
                    });

                // Central panel (plugin's GUI):
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    egui::ScrollArea::both().auto_shrink([false, false]).show(
                        ui,
                        |ui| match &*arcs.dsp_state.read().unwrap() {
                            DspState::NoDsp => {
                                ui.label("-- No DSP --");
                            }
                            DspState::Failed(faust_err_msg) => {
                                ui.colored_label(egui::Color32::LIGHT_RED, faust_err_msg);
                            }
                            DspState::Loaded(dsp) => {
                                ui.style_mut().wrap = Some(false);
                                let margin = egui::Margin {
                                    left: 0.0,
                                    right: 5.0,
                                    top: 0.0,
                                    bottom: 8.0,
                                };
                                egui::Frame::default().outer_margin(margin).show(ui, |ui| {
                                    dsp.with_widgets_mut(|widgets| {
                                        faust_jit_egui::faust_widgets_ui(ui, widgets)
                                    })
                                });
                            }
                        },
                    );
                });
            }
        },
    )
}

fn enum_combobox<T: strum::IntoEnumIterator + PartialEq + std::fmt::Debug>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    selected: &mut T,
) {
    egui::ComboBox::from_id_source(id)
        .selected_text(format!("{:?}", selected))
        .show_ui(ui, |ui| {
            for variant in <T as strum::IntoEnumIterator>::iter() {
                let s = format!("{:?}", variant);
                ui.selectable_value(selected, variant, s);
            }
        });
}

fn top_panel_contents(
    ui: &mut egui::Ui,
    arcs: &EditorArcs,
    async_executor: &AsyncExecutor<crate::NihFaustJit>,
    ed_state: &mut EditorState,
) {
    // Setting the DSP type and (if applicable) number of voices:

    let mut nvoices = *arcs.dsp_nvoices.read().unwrap();
    let mut selected_dsp_type = DspLoadingMode::from_nvoices(nvoices);
    let last_dsp_type = selected_dsp_type;
    ui.horizontal(|ui| {
        ui.label("DSP type:");
        enum_combobox(ui, "dsp-type-combobox", &mut selected_dsp_type);
        match selected_dsp_type {
            DspLoadingMode::AutoDetect => {
                nvoices = -1;
                ui.label("DSP type and number of voices will be detected from script metadata");
            }
            DspLoadingMode::Effect => {
                nvoices = 0;
                ui.label("DSP will be loaded as monophonic effect");
            }
            DspLoadingMode::Instrument => {
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
    *arcs.dsp_nvoices.write().unwrap() = nvoices;

    let mut selected_paths = arcs.selected_paths.write().unwrap();

    // Setting the Faust libraries path:

    ui.label(format!(
        "Faust DSP libraries path: {}",
        selected_paths.dsp_lib_path.display()
    ));
    if (ui.button("Set Faust libraries path")).clicked() {
        let mut dialog =
            egui_file::FileDialog::select_folder(Some(selected_paths.dsp_lib_path.clone()));
        dialog.open();
        ed_state.lib_path_dialog = Some(dialog);
    }
    if let Some(dialog) = &mut ed_state.lib_path_dialog {
        if dialog.show(ui.ctx()).selected() {
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
        ed_state.script_dialog = Some(dialog);
    }
    if let Some(dialog) = &mut ed_state.script_dialog {
        if dialog.show(ui.ctx()).selected() {
            if let Some(file) = dialog.path() {
                selected_paths.dsp_script = Some(file.to_path_buf());
                async_executor.execute_background(crate::Tasks::LoadDsp {
                    restore_zones: false,
                });
            }
        }
    }
}
