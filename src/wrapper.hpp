// This is an hpp file so that bindgen automatically includes C++ types (such as
// bool) used internally by these headers, even though they only export C
// functions.
#include <faust/dsp/libfaust.h>
#include <faust/dsp/dsp.h>
#include <faust/dsp/llvm-dsp.h>

#include <faust/dsp/poly-dsp.h>
#include <iostream>
#include <faust/dsp/poly-llvm-dsp.h>


typedef llvm_dsp_poly_factory W_Factory;
typedef dsp_poly W_Dsp;

W_Factory *w_createDSPFactoryFromFile(const char *filepath, const char *dsp_libs_path, char *err_msg_c);

void w_deleteDSPFactory(W_Factory* factory);

// The `nvoices` parameter can be set to:
//
//   -1 => use the `declare options "[nvoices:xxx]"` metadata in the DSP script.
//   If that script metadata is not present, falls back to the nvoices=0 case.
//   If it is present, see the nvoices=N case.
//
//   0 => the DSP script is considered to be an audio effect (and therefore will
//   be loaded as an always-alive, monophonic DSP)
//
//   N (strictly positive) => the DSP script is considered to be an instrument
//   with a maximum of N simultaneous voices. Setting N=1 for monophonic
//   instruments is perfectly okay. IMPORTANT: If the DSP is actually an effect,
//   that effect will stack as many times as there are MIDI notes being held,
//   and therefore will just emit nothing if no MIDI note is currently being
//   sent. This is _not_ an intended feature of the plugin, just a consequence
//   of how Faust handles polyphony.
//
W_Dsp *w_createDSPInstance(W_Factory *factory, int sample_rate, int nvoices);

struct W_DspInfo {
    int num_inputs;
    int num_outputs;
};

W_DspInfo w_getDSPInfo(W_Dsp *dsp);

void w_computeBuffer(W_Dsp *dsp, int count, float **buf);

void w_deleteDSPInstance(W_Dsp *dsp);

struct W_MidiHandler;

W_MidiHandler *w_buildMidiHandler(W_Dsp *dsp);

void w_deleteMidiHandler(W_MidiHandler *h);

void w_handleMidiEvent(W_MidiHandler *h, double time, const uint8_t bytes[3]);
