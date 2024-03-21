use std::{
    cell::RefCell,
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

/// A vector of pointers (one for each audio buffer), which can be pre-allocated
/// so that the audio thread can just overwrite it and reuse it. It will _only_
/// be used by the process_buffers function, which can only even run on one
/// thread at a time (because it locks the DSP), and which will never try to
/// reuse old pointers from a previous call. So this whole structure behaves as
/// if it was fully local to one process_buffers call (minus its allocation). So
/// marking it as Sync/Send is okay.
struct ChanPtrs {
    vec: RefCell<Vec<*mut f32>>,
}
unsafe impl Sync for ChanPtrs {}
unsafe impl Send for ChanPtrs {}

impl std::fmt::Debug for ChanPtrs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ChanPtrs")
    }
}

#[derive(Debug)]
/// RAII interface to faust DSP factories and instances
pub struct SingletonDsp {
    factory: AtomicPtr<WFactory>,
    /// The DSP instance is mutex-protected, as we don't want its compute
    /// function being called by two threads at the same time
    instance: Mutex<AtomicPtr<WDsp>>,
    uis: AtomicPtr<WUIs>,
    /// The static lifetime here is just to simplify the implementation. It will
    /// never be seen from the outside, as widgets' zones are only valid as long
    /// as the whole SingletonDsp is valid (as they point to values that are
    /// contained inside the WDsp object).
    widgets: RwLock<Vec<DspWidget<&'static mut f32>>>,
    chan_ptrs: ChanPtrs,
    /// Tells how many input & output audio channels this DSP expects
    pub info: WDspInfo,
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
            chan_ptrs: ChanPtrs {
                vec: RefCell::new(vec![]),
            },
            info: WDspInfo {
                num_inputs: 0,
                num_outputs: 0,
            },
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
            this.info = unsafe { w_getDSPInfo(inst_ptr) };
            *this.chan_ptrs.vec.get_mut() =
                vec![null_mut(); this.info.num_inputs.max(this.info.num_outputs) as usize];
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

    /// Modifies _in place_ the given channels. Should be called _after_ all
    /// MIDI events for the current audio buffer have been given to
    /// handle_midi_event.
    ///
    /// If another thread is already calling process_buffers, this will wait
    /// until it terminates.
    ///
    /// The number of expected channels is max(self.info.num_inputs,
    /// self.info.num_outputs):
    ///
    ///   - if audio_bufs contains MORE channels, the excess channels will be
    ///     ignored (ie. will stay untouched)
    ///   - if audio_bufs contains LESS channels, this function will panic
    pub fn process_buffers(&self, audio_bufs: &mut [&mut [f32]]) {
        // First thing to do is to lock the DSP:
        let dsp = self.instance.lock().unwrap();
        let mut ptr_vec = self.chan_ptrs.vec.borrow_mut();
        let samples = audio_bufs[0].len() as i32;
        for i in 0..ptr_vec.len() {
            ptr_vec[i] = audio_bufs[i].as_mut_ptr()
        }
        unsafe {
            w_computeBuffer(dsp.load(Ordering::Relaxed), samples, ptr_vec.as_mut_ptr());
        }
    }
}
