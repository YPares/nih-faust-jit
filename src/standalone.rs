use nih_faust_stereo_fx_jit::NihFaustStereoFxJit;
use nih_plug::nih_export_standalone;

fn main() {
   nih_export_standalone::<NihFaustStereoFxJit>();
}