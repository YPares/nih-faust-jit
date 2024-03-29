use nih_plug::{
    log::{log, Level},
    midi::MidiResult,
    prelude::*,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc, RwLock},
};

mod editor;

#[derive(Debug)]
enum DspState {
    NoDspScript,
    Loaded(faust_jit::SingletonDsp),
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
    nih_egui_state: Arc<nih_plug_egui::EguiState>,

    #[persist = "selected-paths"]
    selected_paths: Arc<RwLock<SelectedPaths>>,

    #[persist = "dsp-nvoices"]
    dsp_nvoices: Arc<RwLock<i32>>,
}

impl NihFaustJit {
    /// Clone from the plugin the Arcs that the GUI thread will need
    pub(crate) fn editor_arcs(&self) -> editor::EditorArcs {
        editor::EditorArcs {
            nih_egui_state: Arc::clone(&self.params.nih_egui_state),
            selected_paths: Arc::clone(&self.params.selected_paths),
            dsp_state: Arc::clone(&self.dsp_state),
            dsp_nvoices: Arc::clone(&self.params.dsp_nvoices),
        }
    }
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

            nih_egui_state: nih_plug_egui::EguiState::from_size(800, 700),

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

#[derive(Debug, PartialEq, Eq, Clone, Copy, strum_macros::EnumIter)]
// We don't reuse faust_jit::DspLoadMode because we need a pure enum here
pub enum DspType {
    AutoDetect,
    Effect,
    Instrument,
}

impl DspType {
    fn from_nvoices(nvoices: i32) -> Self {
        match faust_jit::DspLoadMode::from_nvoices(nvoices) {
            faust_jit::DspLoadMode::AutoDetect => DspType::AutoDetect,
            faust_jit::DspLoadMode::Effect => DspType::Effect,
            faust_jit::DspLoadMode::Instrument { nvoices: _ } => DspType::Instrument,
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

        let cache_folder = env!("LLVM_CACHE_FOLDER"); // Build-time env var
        let opt_cache = if cache_folder.is_empty() {
            None
        } else {
            log!(Level::Info, "Caching llvm bytecode in {}", cache_folder);
            Some(faust_jit::Cache::new(PathBuf::from(cache_folder)))
        };

        Box::new(move |task| match task {
            Tasks::ReloadDsp => {
                let sample_rate = sample_rate_arc.load(Ordering::Relaxed);
                let selected_paths = selected_paths_arc.read().unwrap();
                let dsp_nvoices = *dsp_nvoices_arc.read().unwrap();
                let new_dsp_state = match &selected_paths.dsp_script {
                    Some(script_path) => {
                        match faust_jit::SingletonDsp::from_file(
                            opt_cache.as_ref(),
                            script_path,
                            &[&selected_paths.dsp_lib_path],
                            sample_rate as i32,
                            &faust_jit::DspLoadMode::from_nvoices(dsp_nvoices),
                        ) {
                            Err(msg) => DspState::Failed(msg),
                            Ok(dsp) => {
                                if dsp.info.num_inputs <= 2 && dsp.info.num_outputs <= 2 {
                                    DspState::Loaded(dsp)
                                } else {
                                    DspState::Failed(
                                        format!("DSP has {} input and {} output channels. Max is 2 for each", dsp.info.num_inputs, dsp.info.num_outputs)
                                    )
                                }
                            }
                        }
                    }
                    None => DspState::NoDspScript,
                };
                log!(
                    Level::Debug,
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
        editor::create_editor(self.editor_arcs(), async_executor)
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
            // Handling transport & clock:
            let tp = process_ctx.transport();
            let opt_clock_data = match (tp.tempo, tp.pos_samples()) {
                (Some(tempo), Some(next_buffer_sample_position)) => Some(faust_jit::ClockData {
                    tempo,
                    next_buffer_size: buffer.samples(),
                    next_buffer_sample_position,
                }),
                _ => None,
            };
            dsp.handle_midi_sync(tp.playing, &opt_clock_data);

            // Handling MIDI events:
            while let Some(midi_event) = process_ctx.next_event() {
                let time = midi_event.timing() as f64;
                match midi_event.as_midi() {
                    None | Some(MidiResult::SysEx(_, _)) => { /* We ignore SysEx messages */ }
                    Some(MidiResult::Basic(bytes)) => dsp.handle_raw_midi(time, bytes),
                }
            }

            // Processing audio buffers:
            dsp.process_buffers(buffer.as_slice());
        }
        // Applying Gain parameter:
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
