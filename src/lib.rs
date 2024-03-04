use std::sync::{Arc, Mutex, RwLock};

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};

use egui_file::FileDialog;

use serde::{Deserialize, Serialize};

use faust::SingletonDsp;

pub mod faust;

enum DspState {
    NoDspScript,
    Loaded(SingletonDsp),
    Failed(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct SelectedPaths {
    dsp_script: Option<std::path::PathBuf>,
    dsp_lib_path: std::path::PathBuf,
}

impl DspState {
    /// Try to load a SingletonDsp from the currently selected paths
    fn from_paths(paths: &SelectedPaths, sample_rate: f32) -> Self {
        match &paths.dsp_script {
            Some(script_path) => {
                match SingletonDsp::from_file(
                    script_path.to_str().unwrap(),
                    paths.dsp_lib_path.to_str().unwrap(),
                    sample_rate,
                ) {
                    Err(msg) => Self::Failed(msg),
                    Ok(dsp) => Self::Loaded(dsp),
                }
            }
            None => Self::NoDspScript,
        }
    }
}

pub struct NihFaustStereoFxJit {
    sample_rate: f32,
    params: Arc<NihFaustStereoFxJitParams>,
    dsp_state: Arc<Mutex<DspState>>,
}

#[derive(Params)]
struct NihFaustStereoFxJitParams {
    #[id = "gain"]
    pub gain: FloatParam,

    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[persist = "selected-paths"]
    selected_paths: Arc<RwLock<SelectedPaths>>,
}

impl Default for NihFaustStereoFxJit {
    fn default() -> Self {
        Self {
            sample_rate: 0.0,
            params: Arc::new(NihFaustStereoFxJitParams::default()),
            dsp_state: Arc::new(Mutex::new(DspState::NoDspScript)),
        }
    }
}

impl Default for NihFaustStereoFxJitParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new("Gain", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),

            editor_state: EguiState::from_size(800, 700),

            selected_paths: Arc::new(RwLock::new(SelectedPaths {
                dsp_script: None,
                dsp_lib_path: env!("DSP_LIBS_PATH").into(),
            })),
        }
    }
}

struct GuiState {
    dsp_script_dialog: Option<FileDialog>,
    dsp_lib_path_dialog: Option<FileDialog>,
    selected_paths: Arc<RwLock<SelectedPaths>>,
    dsp_state: Arc<Mutex<DspState>>,
}

impl Plugin for NihFaustStereoFxJit {
    const NAME: &'static str = "Nih Faust Stereo Fx Jit";
    const VENDOR: &'static str = "Yves Pares";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "yves.pares@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),

        aux_input_ports: &[],
        aux_output_ports: &[],

        // Individual ports and the layout as a whole can be named here. By default these names
        // are generated as needed. This layout will be called 'Stereo', while a layout with
        // only one input and output channel would be called 'Mono'.
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate = buffer_config.sample_rate;
        *self.dsp_state.lock().unwrap() = DspState::from_paths(
            &self.params.selected_paths.read().unwrap(),
            self.sample_rate,
        );
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let init_gui_state = GuiState {
            dsp_script_dialog: None,
            dsp_lib_path_dialog: None,
            selected_paths: self.params.selected_paths.clone(),
            dsp_state: self.dsp_state.clone(),
        };
        // We copy sample_rate in the stack so it can be moved in the closure
        // below:
        let sample_rate = self.sample_rate;
        create_egui_editor(
            self.params.editor_state.clone(),
            init_gui_state,
            |_, _| {},
            move |egui_ctx, _setter, gui_state| {
                let mut should_reload = false;
                let mut selected_paths = gui_state.selected_paths.write().unwrap();

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    match &selected_paths.dsp_script {
                        Some(path) => ui.label(format!("DSP script: {}", path.display())),
                        None => ui.colored_label(egui::Color32::YELLOW, "No DSP script selected"),
                    };
                    if (ui.button("Set DSP script")).clicked() {
                        let mut dialog = FileDialog::open_file(selected_paths.dsp_script.clone());
                        dialog.open();
                        gui_state.dsp_script_dialog = Some(dialog);
                    }
                    if let Some(dialog) = &mut gui_state.dsp_script_dialog {
                        if dialog.show(egui_ctx).selected() {
                            if let Some(file) = dialog.path() {
                                selected_paths.dsp_script = Some(file.to_path_buf());
                                should_reload = true;
                            }
                        }
                    }

                    ui.label(format!(
                        "Faust DSP libraries path: {}",
                        selected_paths.dsp_lib_path.display()
                    ));
                    if (ui.button("Set Faust libraries path")).clicked() {
                        let mut dialog =
                            FileDialog::select_folder(Some(selected_paths.dsp_lib_path.clone()));
                        dialog.open();
                        gui_state.dsp_lib_path_dialog = Some(dialog);
                    }
                    if let Some(dialog) = &mut gui_state.dsp_lib_path_dialog {
                        if dialog.show(egui_ctx).selected() {
                            if let Some(file) = dialog.path() {
                                selected_paths.dsp_lib_path = file.to_path_buf();
                                should_reload = true;
                            }
                        }
                    }

                    if let DspState::Failed(faust_err_msg) = &*gui_state.dsp_state.lock().unwrap() {
                        ui.colored_label(egui::Color32::LIGHT_RED, faust_err_msg);
                    }
                });

                // DSP loading is done as part of the GUI thread. Not ideal if
                // the JIT compilation takes time, would be better to spawn a
                // thread to do the reload
                if should_reload {
                    // We swap the current DSP state with the new one. This
                    // calls drop on the current DSP if it was loaded:
                    *gui_state.dsp_state.lock().unwrap() =
                        DspState::from_paths(&selected_paths, sample_rate);
                }
            },
        )
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if let DspState::Loaded(dsp) = &mut *self.dsp_state.lock().unwrap() {
            dsp.process_buffer(buffer);
        }

        for channel_samples in buffer.iter_samples() {
            let gain = self.params.gain.smoothed.next();

            for sample in channel_samples {
                *sample *= gain;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for NihFaustStereoFxJit {
    const CLAP_ID: &'static str = "com.your-domain.nih-faust-stereo-fx-jit";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Loading Faust DSP scripts");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}

impl Vst3Plugin for NihFaustStereoFxJit {
    const VST3_CLASS_ID: [u8; 16] = *b"NihFaustStereoFx";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_clap!(NihFaustStereoFxJit);
nih_export_vst3!(NihFaustStereoFxJit);
