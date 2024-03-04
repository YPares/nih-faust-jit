use std::sync::{atomic::Ordering, Arc, Mutex, RwLock};

use nih_plug::{
    log::{log, Level},
    prelude::*,
};
use nih_plug_egui::{create_egui_editor, egui, EguiState};

use egui_file::FileDialog;

use serde::{Deserialize, Serialize};

use faust::SingletonDsp;

pub mod faust;

#[derive(Debug)]
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

pub struct NihFaustStereoFxJit {
    sample_rate: Arc<AtomicF32>,
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
            sample_rate: Arc::new(AtomicF32::new(0.0)),
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

pub enum Tasks {
    ReloadDsp,
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

    type SysExMessage = ();

    type BackgroundTask = Tasks;

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let sample_rate = Arc::clone(&self.sample_rate);
        // This function may be called before self.sample_rate has been properly
        // initialized, and the task executor closure cannot borrow self. This
        // is why the sample rate is stored in an Arc<AtomicF32> which we can
        // read later, when it is actually time to load a DSP
        let selected_paths = Arc::clone(&self.params.selected_paths);
        let dsp_state = Arc::clone(&self.dsp_state);
        Box::new(move |task| match task {
            Tasks::ReloadDsp => {
                let cur_paths = &selected_paths.read().unwrap();
                let cur_sr = sample_rate.load(Ordering::Relaxed);
                let new_dsp_state = match &cur_paths.dsp_script {
                    Some(script_path) => {
                        match SingletonDsp::from_file(
                            script_path.to_str().unwrap(),
                            cur_paths.dsp_lib_path.to_str().unwrap(),
                            cur_sr,
                        ) {
                            Err(msg) => DspState::Failed(msg),
                            Ok(dsp) => DspState::Loaded(dsp),
                        }
                    }
                    None => DspState::NoDspScript,
                };
                log!(
                    Level::Info,
                    "Loaded {:?} with SR {} => {:?}",
                    cur_paths,
                    cur_sr,
                    new_dsp_state
                );
                *dsp_state.lock().unwrap() = new_dsp_state;
            }
        })
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate
            .store(buffer_config.sample_rate, Ordering::Relaxed);
        context.execute(Tasks::ReloadDsp);
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let init_gui_state = GuiState {
            dsp_script_dialog: None,
            dsp_lib_path_dialog: None,
            selected_paths: Arc::clone(&self.params.selected_paths),
            dsp_state: Arc::clone(&self.dsp_state),
        };
        create_egui_editor(
            Arc::clone(&self.params.editor_state),
            init_gui_state,
            |_, _| {},
            move |egui_ctx, _setter, gui_state| {
                let mut should_reload = false;

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    let mut selected_paths = gui_state.selected_paths.write().unwrap();

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

                if should_reload {
                    async_executor.execute_background(Tasks::ReloadDsp);
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
