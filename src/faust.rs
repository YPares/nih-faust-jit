use nih_plug::{
    buffer::Buffer,
    context::process::ProcessContext,
    //log::{log, Level},
    midi::MidiResult,
    plugin::Plugin,
};
use std::{
    ffi::{c_void, CStr, CString},
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};

mod c {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

    // Functions prefixed with 'w_' are those defined here in wrapper.cpp
}

#[derive(Debug)]
/// RAII interface to faust DSP factories and instances
pub struct SingletonDsp {
    factory: AtomicPtr<c::llvm_dsp_factory>,
    instance: AtomicPtr<c::llvm_dsp>,
    midi_handler: AtomicPtr<c::W_MidiHandler>,
}

impl Drop for SingletonDsp {
    fn drop(&mut self) {
        unsafe {
            let midi_handler = self.midi_handler.get_mut();
            if !midi_handler.is_null() {
                c::w_deleteMidiHandler(*midi_handler);
            }
            let instance = self.instance.get_mut();
            if !instance.is_null() {
                c::w_deleteDSPInstance(*instance);
            }
            let factory = self.factory.get_mut();
            if !factory.is_null() {
                c::deleteDSPFactory(*factory);
            }
        }
    }
}

impl SingletonDsp {
    /// Load a faust .dsp file and initialize the DSP
    pub fn from_file(
        script_path: &str,
        dsp_libs_path: &str,
        sample_rate: f32,
    ) -> Result<Self, String> {
        let mut this = Self {
            factory: AtomicPtr::new(null_mut()),
            instance: AtomicPtr::new(null_mut()),
            midi_handler: AtomicPtr::new(null_mut()),
        };
        let [script_path_c, dsp_libs_path_c] = [script_path, dsp_libs_path]
            .map(|s| CString::new(s).expect(&format!("{} failed to convert to CString", s)));
        let mut error_msg_buf = [0; 4096];
        let fac_ptr = unsafe {
            c::w_createDSPFactoryFromFile(
                script_path_c.as_ptr(),
                dsp_libs_path_c.as_ptr(),
                error_msg_buf.as_mut_ptr(),
            )
        };

        if fac_ptr.is_null() {
            let error_msg = unsafe { CStr::from_ptr(error_msg_buf.as_ptr()) };
            Err(error_msg
                .to_str()
                .map_err(|s| format!("Could not parse Faust err msg as utf8: {}", s))?
                .to_string())
        } else {
            *this.factory.get_mut() = fac_ptr;
            let inst_ptr = unsafe { c::llvm_dsp_factory_createDSPInstance(fac_ptr as *mut c_void) };
            unsafe {
                c::llvm_dsp_init(inst_ptr as *mut c_void, sample_rate as i32);
            };
            *this.instance.get_mut() = inst_ptr;
            let num_inputs = unsafe { c::llvm_dsp_getNumInputs(inst_ptr as *mut c_void) };
            let num_outputs = unsafe { c::llvm_dsp_getNumOutputs(inst_ptr as *mut c_void) };
            if num_inputs <= 2 && num_outputs <= 2 {
                *this.midi_handler.get_mut() = unsafe { c::w_buildMidiUI(inst_ptr) };
                Ok(this)
            } else {
                Err(format!(
                    "DSP has {} input and {} output audio channels. Max is 2 for each",
                    num_inputs, num_outputs
                ))
            }
        }
    }

    pub fn process_buffer<T: Plugin>(
        &mut self,
        audio_buf: &mut Buffer,
        process_ctx: &mut impl ProcessContext<T>,
    ) {
        //log!(Level::Debug, "process_buffer called with {} samples", audio_buf.samples());

        // Handling MIDI events:
        while let Some(midi_event) = process_ctx.next_event() {
            //log!(Level::Debug, "Received: {:?}", midi_event);
            let time = midi_event.timing() as f64;
            match midi_event.as_midi() {
                None | Some(MidiResult::SysEx(_, _)) => {
                    //log!(Level::Debug, "Ignored midi_event");
                }
                Some(MidiResult::Basic(bytes)) => {
                    // Faust expects status (type) bits _not_ to be shifted, so
                    // we leave status bits in place and just set the other ones
                    // to zero:
                    let status = (bytes[0] & 0b11110000) as i32;
                    let channel = (bytes[0] & 0b00001111) as i32;
                    // Is program change or aftertouch:
                    let has_two_data_bytes = status == 0b11000000 || status == 0b11010000;
                    let handler = self.midi_handler.load(Ordering::Relaxed);
                    unsafe {
                        if has_two_data_bytes {
                            //log!(Level::Debug, "Calling handleData1");
                            c::w_handleData1(handler, time, status, channel, bytes[1] as i32);
                        } else {
                            //log!(Level::Debug, "Calling handleData2");
                            c::w_handleData2(
                                handler,
                                time,
                                status,
                                channel,
                                bytes[1] as i32,
                                bytes[2] as i32,
                            );
                        }
                    }
                }
            }
        }

        let buf_slice = audio_buf.as_slice();
        let mut buf_ptrs = [buf_slice[0].as_mut_ptr(), buf_slice[1].as_mut_ptr()];
        // We used --in-place when creating the DSP, so input and output should
        // be the same pointer
        unsafe {
            c::llvm_dsp_compute(
                self.instance.load(Ordering::Relaxed) as *mut c_void,
                audio_buf.samples() as i32,
                buf_ptrs.as_mut_ptr(),
                buf_ptrs.as_mut_ptr(),
            );
        }
    }
}
