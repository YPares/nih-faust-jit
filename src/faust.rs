use nih_plug::buffer::Buffer;
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
}

impl Drop for SingletonDsp {
    fn drop(&mut self) {
        unsafe {
            let inst = self.instance.get_mut();
            if !inst.is_null() {
                c::w_deleteDSPInstance(*inst);
            }
            let fact = self.factory.get_mut();
            if !fact.is_null() {
                c::deleteDSPFactory(*fact);
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
            let is_stereo = unsafe {
                c::llvm_dsp_getNumInputs(inst_ptr as *mut c_void) == 2
                    && c::llvm_dsp_getNumOutputs(inst_ptr as *mut c_void) == 2
            };
            if is_stereo {
                Ok(this)
            } else {
                Err("DSP must have 2 input & 2 output chans".to_string())
            }
        }
    }

    pub fn process_buffer(&mut self, buf: &mut Buffer) {
        //log!(Level::Trace, "process_buffer called with {} samples", buf.samples());
        let buf_slice = buf.as_slice();
        let mut buf_ptrs = [buf_slice[0].as_mut_ptr(), buf_slice[1].as_mut_ptr()];
        // We used --in-place when creating the DSP, so input and output should
        // be the same pointer
        unsafe {
            c::llvm_dsp_compute(
                self.instance.load(Ordering::Relaxed) as *mut c_void,
                buf.samples() as i32,
                buf_ptrs.as_mut_ptr(),
                buf_ptrs.as_mut_ptr(),
            );
        }
    }
}
