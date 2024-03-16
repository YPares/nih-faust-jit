use nih_plug::{
    log::{log, Level},
    prelude::*,
};
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use serde::{Deserialize, Serialize};
use std::sync::{atomic::Ordering, Arc, RwLock};
use strum_macros::EnumIter;

use faust::SingletonDsp;
use gui::*;

pub mod faust;
pub mod gui;

#[derive(Debug)]
enum DspState {
    NoDspScript,
    Loaded(SingletonDsp),
    Failed(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectedPaths {
    dsp_script: Option<std::path::PathBuf>,
    dsp_lib_path: std::path::PathBuf,
}

pub struct NihFaustJit {
    sample_rate: Arc<AtomicF32>,
    params: Arc<NihFaustJitParams>,
    dsp_state: Arc<RwLock<DspState>>,
}

#[derive(Params)]
struct NihFaustJitParams {
    #[id = "gain"]
    pub gain: FloatParam,

    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[persist = "selected-paths"]
    selected_paths: Arc<RwLock<SelectedPaths>>,

    #[persist = "dsp-nvoices"]
    dsp_nvoices: Arc<RwLock<i32>>,
}

impl Default for NihFaustJit {
    fn default() -> Self {
        Self {
            sample_rate: Arc::new(AtomicF32::new(0.0)),
            params: Arc::new(NihFaustJitParams::default()),
            dsp_state: Arc::new(RwLock::new(DspState::NoDspScript)),
        }
    }
}

impl Default for NihFaustJitParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new("Gain", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),

            editor_state: EguiState::from_size(800, 700),

            selected_paths: Arc::new(RwLock::new(SelectedPaths {
                dsp_script: None,
                dsp_lib_path: env!("DSP_LIBS_PATH").into(),
            })),

            dsp_nvoices: Arc::new(RwLock::new(-1)),
        }
    }
}

pub enum Tasks {
    ReloadDsp,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, EnumIter)]
pub enum DspType {
    AutoDetect,
    Effect,
    Instrument,
}

impl DspType {
    fn from_nvoices(nvoices: i32) -> Self {
        match nvoices {
            -1 => DspType::AutoDetect,
            0 => DspType::Effect,
            n => {
                assert!(n > 0, "nvoices must be >= -1");
                DspType::Instrument
            }
        }
    }
}

impl Plugin for NihFaustJit {
    const NAME: &'static str = "nih-faust-jit";
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

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();

    type BackgroundTask = Tasks;

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let sample_rate_arc = Arc::clone(&self.sample_rate);
        // This function may be called before self.sample_rate has been properly
        // initialized, and the task executor closure cannot borrow self. This
        // is why the sample rate is stored in an Arc<AtomicF32> which we can
        // read later, when it is actually time to load a DSP

        let selected_paths_arc = Arc::clone(&self.params.selected_paths);
        let dsp_nvoices_arc = Arc::clone(&self.params.dsp_nvoices);
        let dsp_state_arc = Arc::clone(&self.dsp_state);

        Box::new(move |task| match task {
            Tasks::ReloadDsp => {
                let sample_rate = sample_rate_arc.load(Ordering::Relaxed);
                let selected_paths = &selected_paths_arc.read().unwrap();
                let dsp_nvoices = *dsp_nvoices_arc.read().unwrap();
                let new_dsp_state = match &selected_paths.dsp_script {
                    Some(script_path) => {
                        match SingletonDsp::from_file(
                            script_path.to_str().unwrap(),
                            selected_paths.dsp_lib_path.to_str().unwrap(),
                            sample_rate,
                            dsp_nvoices,
                        ) {
                            Err(msg) => DspState::Failed(msg),
                            Ok(dsp) => {
                                log!(Level::Debug, "Widgets: {:?}", dsp.widgets().lock().unwrap());
                                DspState::Loaded(dsp)
                            }
                        }
                    }
                    None => DspState::NoDspScript,
                };
                log!(
                    Level::Info,
                    "Loaded {:?} with sample_rate={}, nvoices={} => {:?}",
                    selected_paths,
                    sample_rate,
                    dsp_nvoices,
                    new_dsp_state
                );
                // This is the only place where the whole DSP state is locked in
                // write mode, and only so we can swap it with the newly loaded
                // one:
                *dsp_state_arc.write().unwrap() = new_dsp_state;
            }
        })
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        init_ctx: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate
            .store(buffer_config.sample_rate, Ordering::Relaxed);
        init_ctx.execute(Tasks::ReloadDsp);
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let selected_paths_arc = Arc::clone(&self.params.selected_paths);
        let dsp_nvoices_arc = Arc::clone(&self.params.dsp_nvoices);
        let dsp_state_arc = Arc::clone(&self.dsp_state);
        let editor_state_arc = Arc::clone(&self.params.editor_state);

        create_egui_editor(
            Arc::clone(&self.params.editor_state),
            (None, None),
            |_, _| {},
            move |egui_ctx, _param_setter, (dsp_script_dialog, dsp_lib_path_dialog)| {
                if editor_state_arc.is_open() {
                    // Top panel (DSP settings, loading):
                    egui::TopBottomPanel::top("DSP loading")
                        .frame(egui::Frame::default().inner_margin(8.0))
                        .show(egui_ctx, |ui| {
                            top_panel_contents(
                                &async_executor,
                                egui_ctx,
                                ui,
                                &dsp_nvoices_arc,
                                &selected_paths_arc,
                                dsp_lib_path_dialog,
                                dsp_script_dialog,
                            );
                        });

                    // Central panel (plugin's GUI):
                    egui::CentralPanel::default()
                        .frame(egui::Frame::default().inner_margin(8.0))
                        .show(egui_ctx, |ui| {
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| match &*dsp_state_arc.read().unwrap() {
                                    DspState::NoDspScript => {
                                        ui.label("-- No DSP --");
                                    }
                                    DspState::Failed(faust_err_msg) => {
                                        ui.colored_label(egui::Color32::LIGHT_RED, faust_err_msg);
                                    }
                                    DspState::Loaded(dsp) => {
                                        central_panel_contents(
                                            ui,
                                            &mut *dsp.widgets().lock().unwrap(),
                                        );
                                    }
                                });
                        });
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
        process_ctx: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if let DspState::Loaded(dsp) = &*self.dsp_state.read().unwrap() {
            dsp.process_buffer(buffer, process_ctx);
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

impl ClapPlugin for NihFaustJit {
    const CLAP_ID: &'static str = "com.ypares.nih-faust-jit";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Using jit-compiled Faust DSP scripts");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Instrument,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for NihFaustJit {
    const VST3_CLASS_ID: [u8; 16] = *b"nih-faust-jit-yp";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Stereo,
    ];
}

nih_export_clap!(NihFaustJit);
nih_export_vst3!(NihFaustJit);
