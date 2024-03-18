use nih_plug::{
    buffer::Buffer, context::process::ProcessContext, midi::MidiResult, plugin::Plugin,
};
use std::{
    ffi::{c_void, CStr, CString},
    ptr::null_mut,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Mutex,
    },
};

use widgets::*;
use wrapper::*;

pub mod widgets;
pub mod wrapper;

#[derive(Debug)]
/// RAII interface to faust DSP factories and instances
pub struct SingletonDsp {
    factory: AtomicPtr<WFactory>,
    /// The DSP instance is mutex-protected, as we don't want its compute
    /// function being called by two threads at the same time
    instance: Mutex<AtomicPtr<WDsp>>,
    uis: AtomicPtr<WUIs>,
    widgets: Mutex<Vec<DspWidget<'static>>>,
}
// AtomicPtr is used above only to make the pointers (and thus the whole type)
// Sync. The pointers themselves will never be mutated.

impl Drop for SingletonDsp {
    fn drop(&mut self) {
        unsafe {
            let instance = self.instance.get_mut().unwrap().get_mut();
            if !instance.is_null() {
                w_deleteDSPInstance(*instance);
            }
            let factory = self.factory.get_mut();
            if !factory.is_null() {
                w_deleteDSPFactory(*factory);
            }
            let uis = self.uis.get_mut();
            if !uis.is_null() {
                w_deleteUIs(*uis);
            }
        }
    }
}

impl SingletonDsp {
    /// Load a faust .dsp file and initialize the DSP
    ///
    /// nvoices controls both the amount of voices and the type of DSP that will
    /// be loaded. See w_createDSPInstance for more info.
    pub fn from_file(
        script_path: &str,
        dsp_libs_path: &str,
        sample_rate: f32,
        nvoices: i32,
    ) -> Result<Self, String> {
        let mut this = Self {
            factory: AtomicPtr::new(null_mut()),
            instance: Mutex::new(AtomicPtr::new(null_mut())),
            uis: AtomicPtr::new(null_mut()),
            widgets: Mutex::new(vec![]),
        };
        let [script_path_c, dsp_libs_path_c] = [script_path, dsp_libs_path]
            .map(|s| CString::new(s).expect(&format!("{} failed to convert to CString", s)));
        let mut error_msg_buf = [0; 4096];
        let fac_ptr = unsafe {
            w_createDSPFactoryFromFile(
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
            let inst_ptr = unsafe { w_createDSPInstance(fac_ptr, sample_rate as i32, nvoices) };
            *this.instance.get_mut().unwrap().get_mut() = inst_ptr;
            let info = unsafe { w_getDSPInfo(inst_ptr) };
            if info.num_inputs <= 2 && info.num_outputs <= 2 {
                let mut gui_builder = DspWidgetsBuilder::new();
                *this.uis.get_mut() = unsafe {
                    w_createUIs(
                        inst_ptr,
                        Some(widget_decl_callback),
                        (&mut gui_builder) as *mut DspWidgetsBuilder as *mut c_void,
                    )
                };
                gui_builder.build_widgets(this.widgets.get_mut().unwrap());
                assert!(
                    gui_builder.has_no_remaining_decls(),
                    "Some widget declarations haven't been consumed"
                );
                Ok(this)
            } else {
                Err(format!(
                    "DSP has {} input and {} output audio channels. Max is 2 for each",
                    info.num_inputs, info.num_outputs
                ))
            }
        }
    }

    pub fn widgets(&self) -> &Mutex<Vec<DspWidget<'static>>> {
        &self.widgets
    }

    pub fn process_buffer<T: Plugin>(
        &self,
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
                    let uis = self.uis.load(Ordering::Relaxed);
                    unsafe {
                        w_handleMidiEvent(uis, time, bytes.as_ptr());
                    }
                }
            }
        }

        let buf_slice = audio_buf.as_slice();
        let mut buf_ptrs = [buf_slice[0].as_mut_ptr(), buf_slice[1].as_mut_ptr()];

        let dsp = self.instance.lock().unwrap();
        unsafe {
            w_computeBuffer(
                dsp.load(Ordering::Relaxed),
                audio_buf.samples() as i32,
                buf_ptrs.as_mut_ptr(),
            );
        }
    }
}
