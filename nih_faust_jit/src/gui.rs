use nih_plug::prelude::*;
use nih_plug_egui::egui;
use std::sync::{Arc, RwLock};

use crate::DspType;

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
