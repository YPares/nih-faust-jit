use std::{
    cell::RefCell,
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        Mutex, RwLock,
    },
};

use wrapper::*;

pub use cache::*;
pub use widgets::*;
pub use wrapper::DspInfo;

mod cache;
mod widgets;
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

/// How to load a DSP
pub enum DspLoadMode {
    /// Use the script metadata
    AutoDetect,
    /// Monophonic (always alive) effect
    Effect,
    /// Polyphonic instrument with max number of voices
    Instrument { nvoices: i32 },
}

impl DspLoadMode {
    pub fn from_nvoices(nvoices: i32) -> Self {
        assert!(nvoices >= -1, "DspLoadMode: nvoices must be >= -1");
        match nvoices {
            -1 => Self::AutoDetect,
            0 => Self::Effect,
            _ => Self::Instrument { nvoices },
        }
    }
    pub fn to_nvoices(&self) -> i32 {
        match self {
            Self::AutoDetect => -1,
            Self::Effect => 0,
            Self::Instrument { nvoices } => *nvoices,
        }
    }
}

#[derive(Debug)]
/// RAII interface to faust DSP factories and instances
pub struct SingletonDsp {
    transport_already_playing: AtomicBool,
    /// The factory pointer is kept around only to be deallocated when its time
    /// to drop the SingletonDsp
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
    /// Tells the sample rate and how many input & output audio channels this
    /// DSP expects
    pub info: DspInfo,
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
            let uis = self.uis.get_mut();
            if !uis.is_null() {
                w_deleteUIs(*uis);
            }
            let factory = self.factory.get_mut();
            if !factory.is_null() {
                w_deleteDSPFactory(*factory);
            }
        }
    }
}

/// Data needed to generate a MIDI clock for the DSP
pub struct ClockData {
    /// The tempo (as given by the host)
    pub tempo: f64,
    /// The buffer size
    pub next_buffer_size: usize,
    /// Where we are in the track (expressed in samples)
    pub next_buffer_sample_position: i64,
}

fn path_to_cstring(p: &Path) -> Result<CString, String> {
    CString::new(p.to_str().ok_or("Path cannot not be converted to string")?)
        .map_err(|e| e.to_string())
}

impl SingletonDsp {
    fn new_empty() -> Self {
        Self {
            transport_already_playing: AtomicBool::new(false),
            factory: AtomicPtr::new(null_mut()),
            instance: Mutex::new(AtomicPtr::new(null_mut())),
            uis: AtomicPtr::new(null_mut()),
            widgets: RwLock::new(vec![]),
            chan_ptrs: ChanPtrs {
                vec: RefCell::new(vec![]),
            },
            info: DspInfo {
                sample_rate: 0,
                num_inputs: 0,
                num_outputs: 0,
            },
        }
    }

    fn add_factory(
        &mut self,
        opt_cache: Option<&Cache>,
        script_path: &Path,
        import_paths: &[&Path],
    ) -> Result<(), String> {
        let mut error_msg_buf = [0; 4096];
        let fac_ptr = match opt_cache {
            Some(cache) => {
                // We are not including import_paths in the hash as it takes too
                // long too hash. Improve this later
                let res_id = Cache::hash_input(script_path, &[]).map_err(|e| e.to_string())?;
                match cache.query(res_id) {
                    CacheCheck::Hit(folder) => unsafe {
                        w_readFactoryFromFolder(
                            path_to_cstring(&folder)?.as_ptr(),
                            error_msg_buf.as_mut_ptr(),
                        )
                    },
                    CacheCheck::Miss(writer) => {
                        let fac_ptr =
                            new_factory_from_file(script_path, import_paths, &mut error_msg_buf)?;
                        writer.with_dest_folder(|folder| {
                            unsafe {
                                w_writeFactoryToFolder(fac_ptr, path_to_cstring(folder)?.as_ptr());
                            };
                            Ok::<_, String>(fac_ptr)
                        })?
                    }
                }
            }
            None => new_factory_from_file(script_path, import_paths, &mut error_msg_buf)?,
        };
        if fac_ptr.is_null() {
            let error_msg = unsafe { CStr::from_ptr(error_msg_buf.as_ptr()) };
            Err(error_msg
                .to_str()
                .map_err(|s| format!("Could not parse Faust err msg as utf8: {}", s))?
                .to_string())
        } else {
            *self.factory.get_mut() = fac_ptr;
            Ok(())
        }
    }

    fn add_instance(&mut self, sample_rate: i32, load_mode: &DspLoadMode) {
        *self.instance.get_mut().unwrap().get_mut() = unsafe {
            w_createDSPInstance(
                *self.factory.get_mut(),
                sample_rate,
                load_mode.to_nvoices(),
                false,
            )
        };
    }

    fn add_info_and_uis(&mut self) {
        let inst_ptr = *self.instance.get_mut().unwrap().get_mut();
        self.info = unsafe { w_getDSPInfo(inst_ptr) };
        *self.chan_ptrs.vec.get_mut() =
            vec![null_mut(); self.info.num_inputs.max(self.info.num_outputs) as usize];
        let mut widgets_builder = DspWidgetsBuilder::new();
        *self.uis.get_mut() = unsafe {
            w_createUIs(
                inst_ptr,
                (&mut widgets_builder) as *mut DspWidgetsBuilder as *mut c_void,
            )
        };
        widgets_builder.build_widgets(self.widgets.get_mut().unwrap());
    }

    /// Load a faust .dsp file and initialize the DSP
    ///
    /// Adds to the import_paths the parent folder of script_path, so that the
    /// script can import other files using paths relative to itself
    ///
    /// Can use a file-based Cache to store the LLVM bytecode to save time when
    /// reloading the same DSP in a future execution. IMPORTANT: That cache
    /// takes into account only the contents of the script, NOT what it imports
    pub fn from_file(
        opt_cache: Option<&Cache>,
        script_path: &Path,
        import_paths: &[&Path],
        sample_rate: i32,
        load_mode: &DspLoadMode,
    ) -> Result<Self, String> {
        let mut dsp = Self::new_empty();
        dsp.add_factory(opt_cache, script_path, import_paths)?;
        dsp.add_instance(sample_rate, load_mode);
        dsp.add_info_and_uis();
        Ok(dsp)
    }

    /// Creates a SingletonDsp from an already created `dsp_poly_factory` (the
    /// Faust C++ class).
    ///
    /// owns_factory tells whether the ownership of the factory is transmitted
    /// to the SingletonDsp, and therefore if the factory should be deleted when
    /// the SingletonDsp goes out of scope. If not, it is up to you to make sure
    /// the factory stays allocated as long as the SingletonDsp is in use.
    ///
    /// This allows to use SingletonDsp and DspWidgets with DSPs/DSP factories
    /// obtained from other means than llvm JIT compilation (for instance a DSP
    /// compiled statically with faust2rust).
    ///
    /// IMPORTANT: If your application already defines the faust static
    /// variables `std::list<GUI *> GUI::fGuiList` and `ztimedmap
    /// GUI::gTimedZoneMap`, do not forget to build this crate WITHOUT the
    /// default features, so it does not try to create them too.
    pub fn from_poly_factory_ptr(
        factory_ptr: *mut WFactory,
        owns_factory: bool,
        sample_rate: i32,
        load_mode: &DspLoadMode,
    ) -> Self {
        let mut dsp = Self::new_empty();
        *dsp.factory.get_mut() = factory_ptr;
        dsp.add_instance(sample_rate, load_mode);
        dsp.add_info_and_uis();
        if !owns_factory {
            // We don't own the factory and therefore don't keep its pointer
            *dsp.factory.get_mut() = null_mut();
        }
        dsp
    }

    /// Creates a SingletonDsp from an already created instance of a subclass of
    /// `dsp` (the Faust C++ class), preferably wrapped in `timed_dsp` to
    /// benefit from timestamping of MIDI events.
    ///
    /// SingletonDsp takes ownership of the `dsp` instance (this is needed to
    /// create & destroy the UIs properly) and WILL delete it when the
    /// SingletonDsp goes out of scope, so you may NOT use the dsp pointer after
    /// calling this function
    ///
    /// See from_poly_factory_ptr doc for more information.
    pub fn from_dsp_ptr(dsp_ptr: *mut WDsp) -> Self {
        let mut dsp = Self::new_empty();
        *dsp.instance.get_mut().unwrap().get_mut() = dsp_ptr;
        dsp.add_info_and_uis();
        dsp
    }

    /// If another thread is currently calling with_widgets_mut, this will wait
    /// until it terminates
    pub fn with_widgets<T>(&self, f: impl FnOnce(&[DspWidget<&mut f32>]) -> T) -> T {
        f(&*self.widgets.read().unwrap())
    }

    /// If another thread is already calling with_widgets_mut, this will wait
    /// until it terminates
    pub fn with_widgets_mut<T>(&self, f: impl FnOnce(&mut [DspWidget<&mut f32>]) -> T) -> T {
        f(&mut *self.widgets.write().unwrap())
    }

    /// To be called for each midi event for the current audio buffer
    pub fn handle_raw_midi(&self, timestamp: f64, midi_data: [u8; 3]) {
        let uis = self.uis.load(Ordering::Relaxed);
        unsafe {
            w_handleRawMidi(uis, timestamp, midi_data.as_ptr());
        }
    }

    /// Send MIDI clock and play/stop messages to the DSP
    pub fn handle_midi_sync(&self, playing: bool, opt_clock_data: &Option<ClockData>) {
        let already_playing = self.transport_already_playing.load(Ordering::Relaxed);
        let uis = self.uis.load(Ordering::Relaxed);
        if playing {
            if !already_playing {
                unsafe { w_handleMidiSync(uis, 0.0, WMidiSyncMsg::MIDI_START) };
                self.transport_already_playing
                    .store(true, Ordering::Relaxed);
            }

            // We generate and send to the DSP a 24 PPQN clock:
            if let Some(clock_data) = opt_clock_data {
                let samples_per_beat = (self.info.sample_rate as f64) * 60.0 / clock_data.tempo;
                let samples_per_pulse = (samples_per_beat / 24.0) as i64;

                // next_pulse_pos is in buffer coordinates (ie. 0 is the first sample of
                // the current buffer)
                let rem = clock_data.next_buffer_sample_position % samples_per_pulse;
                let mut next_pulse_pos = if rem == 0 { 0 } else { samples_per_pulse - rem };
                while next_pulse_pos < clock_data.next_buffer_size as i64 {
                    unsafe {
                        w_handleMidiSync(uis, next_pulse_pos as f64, WMidiSyncMsg::MIDI_CLOCK)
                    };
                    next_pulse_pos += samples_per_pulse;
                }
            }
        } else {
            if already_playing {
                unsafe { w_handleMidiSync(uis, 0.0, WMidiSyncMsg::MIDI_STOP) };
                self.transport_already_playing
                    .store(false, Ordering::Relaxed);
            }
        }
    }

    /// Modifies _in place_ the given channels. Should be called _after_ all
    /// MIDI events for the current audio buffer have been handled.
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
        unsafe { w_updateAllGuis() };

        // First thing to do is to lock the DSP:
        let dsp = self.instance.lock().unwrap();
        let mut ptr_vec = self.chan_ptrs.vec.borrow_mut();
        let samples = audio_bufs[0].len() as i32;
        for i in 0..ptr_vec.len() {
            ptr_vec[i] = audio_bufs[i].as_mut_ptr()
        }
        unsafe {
            w_computeDSP(dsp.load(Ordering::Relaxed), samples, ptr_vec.as_mut_ptr());
        }
    }
}

fn new_factory_from_file(
    script_path: &Path,
    import_paths: &[&Path],
    error_msg_buf: &mut [c_char; 4096],
) -> Result<*mut WFactory, String> {
    let script_parent_folder = script_path
        .parent()
        .ok_or("Parent folder of script couldn't be found")?;
    let script_path = path_to_cstring(script_path)?;
    let mut args = vec![
        c"--in-place".to_owned(),
        c"-I".to_owned(),
        path_to_cstring(script_parent_folder)?,
    ];
    for folder in import_paths {
        args.push(c"-I".to_owned());
        args.push(path_to_cstring(folder)?);
    }
    let mut args_ptrs: Vec<_> = args.iter().map(|cstring| cstring.as_ptr()).collect();
    Ok(unsafe {
        w_createDSPFactoryFromFile(
            script_path.as_ptr(),
            args_ptrs.len() as i32,
            args_ptrs.as_mut_ptr(),
            error_msg_buf.as_mut_ptr(),
        )
    })
}
