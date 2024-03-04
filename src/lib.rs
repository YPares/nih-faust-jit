use std::sync::{Arc, Mutex, RwLock};

use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self},
    EguiState,
};

use egui_file::FileDialog;

use faust::SingletonDsp;

pub mod faust;

pub struct NihFaustStereoFxJit {
    sample_rate: f32,
    params: Arc<NihFaustStereoFxJitParams>,
    dsp: Arc<Mutex<Option<SingletonDsp>>>,
    plugin_state: Arc<RwLock<PluginState>>,
}

#[derive(Params)]
struct NihFaustStereoFxJitParams {
    #[id = "gain"]
    pub gain: FloatParam,

    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
}

struct PluginState {
    current_dsp_script: std::path::PathBuf,
    current_dsp_lib_path: std::path::PathBuf,
}

impl Default for NihFaustStereoFxJit {
    fn default() -> Self {
        Self {
            sample_rate: 0.0,
            params: Arc::new(NihFaustStereoFxJitParams::default()),
            dsp: Arc::new(Mutex::new(None)),
            plugin_state: Arc::new(RwLock::new(PluginState {
                current_dsp_script: DEFAULT_DSP_SCRIPT_PATH.into(),
                current_dsp_lib_path: DEFAULT_DSP_LIBS_PATH.into(),
            })),
        }
    }
}

impl Default for NihFaustStereoFxJitParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new("Gain", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),

            editor_state: EguiState::from_size(800, 700),
        }
    }
}

const DEFAULT_DSP_SCRIPT_PATH: &str = std::env!("DSP_SCRIPT_PATH");
const DEFAULT_DSP_LIBS_PATH: &str = std::env!("DSP_LIBS_PATH");

struct GuiState {
    dsp_script_dialog: Option<FileDialog>,
    dsp_lib_path_dialog: Option<FileDialog>,
    plugin_state: Arc<RwLock<PluginState>>,
    dsp: Arc<Mutex<Option<SingletonDsp>>>,
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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let init_gui_state = GuiState {
            dsp_script_dialog: None,
            dsp_lib_path_dialog: None,
            plugin_state: self.plugin_state.clone(),
            dsp: self.dsp.clone(),
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

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    let mut plugin_state = gui_state
                        .plugin_state
                        .write()
                        .expect("GUI update closure couldn't get PluginState mutex");

                    ui.label(format!(
                        "Loaded DSP script: {}",
                        plugin_state.current_dsp_script.display()
                    ));
                    if (ui.button("Set DSP script")).clicked() {
                        let mut dialog =
                            FileDialog::open_file(Some(plugin_state.current_dsp_script.clone()));
                        dialog.open();
                        gui_state.dsp_script_dialog = Some(dialog);
                    }
                    if let Some(dialog) = &mut gui_state.dsp_script_dialog {
                        if dialog.show(egui_ctx).selected() {
                            if let Some(file) = dialog.path() {
                                plugin_state.current_dsp_script = file.to_path_buf();
                                should_reload = true;
                            }
                        }
                    }

                    ui.label(format!(
                        "Faust DSP libraries path: {}",
                        plugin_state.current_dsp_lib_path.display()
                    ));
                    if (ui.button("Set Faust libraries path")).clicked() {
                        let mut dialog = FileDialog::select_folder(Some(
                            plugin_state.current_dsp_lib_path.clone(),
                        ));
                        dialog.open();
                        gui_state.dsp_lib_path_dialog = Some(dialog);
                    }
                    if let Some(dialog) = &mut gui_state.dsp_lib_path_dialog {
                        if dialog.show(egui_ctx).selected() {
                            if let Some(file) = dialog.path() {
                                plugin_state.current_dsp_lib_path = file.to_path_buf();
                                should_reload = true;
                            }
                        }
                    }
                });

                // DSP loading is done as part of the GUI thread. Not ideal if
                // the loading takes time, would be better to spawn a thread to
                // do the reload
                if should_reload {
                    let plugin_state = gui_state.plugin_state.read().unwrap();
                    let res = SingletonDsp::from_file(
                        plugin_state.current_dsp_script.to_str().unwrap(),
                        plugin_state.current_dsp_lib_path.to_str().unwrap(),
                        sample_rate,
                    );
                    match res {
                        Ok(dsp) => {
                            // We swap the previously loaded DSP with the new
                            // one. This calls drop on the previous DSP:
                            *gui_state.dsp.lock().unwrap() = Some(dsp);
                            println!("Loaded DSP script `{:?}'", plugin_state.current_dsp_script);
                        }
                        Err(msg) => panic!("{}", msg),
                    }
                }
            },
        )
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

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
        let state = self.plugin_state.read().unwrap();
        match SingletonDsp::from_file(
            state.current_dsp_script.to_str().unwrap(),
            state.current_dsp_lib_path.to_str().unwrap(),
            self.sample_rate,
        ) {
            Err(s) => panic!("DSP init failed with: {}", s),
            Ok(dsp) => *self.dsp.lock().unwrap() = Some(dsp),
        };
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if let Some(dsp) = self.dsp.lock().unwrap().as_mut() {
            dsp.compute(buffer);

            for channel_samples in buffer.iter_samples() {
                let gain = self.params.gain.smoothed.next();

                for sample in channel_samples {
                    *sample *= gain;
                }
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
