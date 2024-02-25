use faust::SingletonDsp;
use nih_plug::prelude::*;
use std::sync::{Arc, Mutex};
pub mod faust;

pub struct NihFaustStereoFxJit {
    params: Arc<NihFaustStereoFxJitParams>,
    dsp: Arc<Mutex<SingletonDsp>>,
}

#[derive(Params)]
struct NihFaustStereoFxJitParams {
    #[id = "gain"]
    pub gain: FloatParam,
}

impl Default for NihFaustStereoFxJit {
    fn default() -> Self {
        Self {
            params: Arc::new(NihFaustStereoFxJitParams::default()),
            dsp: Arc::new(Mutex::new(SingletonDsp::default())),
        }
    }
}

impl Default for NihFaustStereoFxJitParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new("Gain", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),
        }
    }
}

const DEFAULT_DSP_SCRIPT_PATH: &str = std::env!("DSP_SCRIPT_PATH");

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
        let mut dsp = self.dsp.lock().expect("Couldn't acquire dsp");
        match dsp.init_from_file(DEFAULT_DSP_SCRIPT_PATH, buffer_config.sample_rate as i32) {
            Err(s) => panic!("DSP init failed with: {}", s),
            _ => true,
        }
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
        let dsp = self.dsp.lock().expect("Couldn't acquire dsp");
        dsp.compute(buffer);

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
