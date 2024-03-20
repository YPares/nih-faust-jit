use std::{
    ffi::{c_void, CStr, CString},
    ptr::null_mut,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Mutex, RwLock,
    },
};

use widgets::*;
use wrapper::*;

pub mod widgets;
mod wrapper;

#[derive(Debug)]
/// RAII interface to faust DSP factories and instances
pub struct SingletonDsp {
    factory: AtomicPtr<WFactory>,
    /// The DSP instance is mutex-protected, as we don't want its compute
    /// function being called by two threads at the same time
    instance: Mutex<AtomicPtr<WDsp>>,
    uis: AtomicPtr<WUIs>,
    widgets: RwLock<Vec<DspWidget<&'static mut f32>>>,
    // This static lifetime here is just to simplify the implementation. It will
    // never be seen from the outside, as widgets' zones are only valid as long
    // as the whole SingletonDsp is valid (as they point to values that are
    // contained inside the WDsp object).
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
            widgets: RwLock::new(vec![]),
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
            let inst_ptr =
                unsafe { w_createDSPInstance(fac_ptr, sample_rate as i32, nvoices, false) };
            *this.instance.get_mut().unwrap().get_mut() = inst_ptr;
            let info = unsafe { w_getDSPInfo(inst_ptr) };
            if info.num_inputs <= 2 && info.num_outputs <= 2 {
                let mut widgets_builder = DspWidgetsBuilder::new();
                *this.uis.get_mut() = unsafe {
                    w_createUIs(
                        inst_ptr,
                        Some(widget_decl_callback),
                        (&mut widgets_builder) as *mut DspWidgetsBuilder as *mut c_void,
                    )
                };
                widgets_builder.build_widgets(this.widgets.get_mut().unwrap());
                Ok(this)
            } else {
                Err(format!(
                    "DSP has {} input and {} output audio channels. Max is 2 for each",
                    info.num_inputs, info.num_outputs
                ))
            }
        }
    }

    /// If another thread is calling with_mut_widgets, this will wait until it
    /// terminates
    pub fn with_widgets<T>(&self, f: impl FnOnce(&[DspWidget<&mut f32>]) -> T) -> T {
        f(&*self.widgets.read().unwrap())
    }

    /// If another thread is already calling with_mut_widgets, this will wait
    /// until it terminates
    pub fn with_widgets_mut<T>(&self, f: impl FnOnce(&mut [DspWidget<&mut f32>]) -> T) -> T {
        f(&mut *self.widgets.write().unwrap())
    }

    /// To be called for each midi event for the current audio buffer
    pub fn handle_midi_event(&self, timestamp: f64, midi_data: [u8; 3]) {
        let uis = self.uis.load(Ordering::Relaxed);
        unsafe {
            w_handleMidiEvent(uis, timestamp, midi_data.as_ptr());
        }
    }

    /// This function should be called _after_ all MIDI events for the current
    /// audio buffer have been sent via handle_midi_event
    ///
    /// If another thread is already calling process_buffers, this will wait
    /// until it terminates
    pub fn process_buffers(&self, left_chan: &mut [f32], right_chan: &mut [f32]) {
        let mut buf_ptrs = [left_chan.as_mut_ptr(), right_chan.as_mut_ptr()];

        let dsp = self.instance.lock().unwrap();
        unsafe {
            w_computeBuffer(
                dsp.load(Ordering::Relaxed),
                left_chan.len() as i32,
                buf_ptrs.as_mut_ptr(),
            );
        }
    }
}
