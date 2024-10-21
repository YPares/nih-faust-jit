use fundsp::hacker::*;
use crate::*;

impl AudioUnit for SingletonDsp {
    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        todo!()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        todo!()
    }

    fn inputs(&self) -> usize {
        self.info.num_inputs as usize
    }

    fn outputs(&self) -> usize {
        self.info.num_outputs as usize
    }

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        SignalFrame::new(self.outputs())
    }

    fn get_id(&self) -> u64 {
        todo!()
    }

    fn footprint(&self) -> usize {
        0
    }
}
