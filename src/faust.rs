#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use nih_plug::buffer::Buffer;
use std::{
    ffi::{CStr, CString},
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct SingletonDsp {
    factory: AtomicPtr<llvm_dsp_factory>,
    instance: AtomicPtr<llvm_dsp>,
}

impl Default for SingletonDsp {
    fn default() -> Self {
        Self {
            factory: AtomicPtr::new(null_mut()),
            instance: AtomicPtr::new(null_mut()),
        }
    }
}

impl Drop for SingletonDsp {
    fn drop(&mut self) {
        self.dealloc_if_needed();
    }
}

impl SingletonDsp {
    fn dealloc_if_needed(&mut self) {
        unsafe {
            let inst = self.instance.get_mut();
            if !inst.is_null() {
                deleteCDSPInstance(*inst);
                *inst = null_mut();
            }
            let fact = self.factory.get_mut();
            if !fact.is_null() {
                deleteCDSPFactory(*fact);
                *fact = null_mut();
            }
        }
    }

    /// Tells if the DSP is ready to process audio
    pub fn ready(&self) -> bool {
        !self.instance.load(Ordering::Relaxed).is_null()
    }

    /// Load a faust .dsp file and initialize the DSP
    pub fn init_from_file(
        &mut self,
        script_path: &str,
        dsp_libs_path: &str,
        sample_rate: f32,
    ) -> Result<(), String> {
        self.dealloc_if_needed();
        let [path_c, target, arg0, arg1, arg2] =
            [script_path, "", "--in-place", "-I", dsp_libs_path]
                .map(|p| CString::new(p).expect(&format!("{} failed to convert to CString", p)));
        let mut arg_ptrs = [arg0.as_ptr(), arg1.as_ptr(), arg2.as_ptr()];
        let mut error_msg_buf = [0; 4096];
        let fac_ptr = unsafe {
            createCDSPFactoryFromFile(
                path_c.as_ptr(),
                arg_ptrs.len() as i32,
                arg_ptrs.as_mut_ptr(),
                target.as_ptr(),
                error_msg_buf.as_mut_ptr(),
                -1,
            )
        };

        if fac_ptr.is_null() {
            let error_msg = unsafe { CStr::from_ptr(error_msg_buf.as_ptr()) };
            Err(error_msg
                .to_str()
                .map_err(|s| format!("Could not parse Faust err msg as utf8: {}", s))?
                .to_string())
        } else {
            *self.factory.get_mut() = fac_ptr;
            let inst_ptr = unsafe { createCDSPInstance(fac_ptr) };
            unsafe {
                initCDSPInstance(inst_ptr, sample_rate as i32);
            };
            *self.instance.get_mut() = inst_ptr;
            let is_stereo = unsafe {
                getNumInputsCDSPInstance(inst_ptr) == 2 && getNumOutputsCDSPInstance(inst_ptr) == 2
            };
            if is_stereo {
                Ok(())
            } else {
                self.dealloc_if_needed();
                Err("DSP must have 2 input & 2 output chans".to_string())
            }
        }
    }

    pub fn compute(&mut self, buf: &mut Buffer) {
        assert!(self.ready(), "DSP not loaded");
        //println!("compute called with {} samples", buf.samples());
        let buf_slice = buf.as_slice();
        let mut buf_ptrs = [buf_slice[0].as_mut_ptr(), buf_slice[1].as_mut_ptr()];
        // We used --in-place when creating the DSP, so input and output should
        // be the same pointer
        unsafe {
            computeCDSPInstance(
                self.instance.load(Ordering::Relaxed),
                buf.samples() as i32,
                buf_ptrs.as_mut_ptr(),
                buf_ptrs.as_mut_ptr(),
            );
        }
    }
}
