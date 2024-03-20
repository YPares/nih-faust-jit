use nih_faust_jit::NihFaustJit;
use nih_plug::nih_export_standalone;

fn main() {
   nih_export_standalone::<NihFaustJit>();
}