// This is an hpp file so that bindgen automatically includes C++ types (such as
// bool) used internally by these headers, even though they only export C
// functions.
#include <faust/dsp/libfaust.h>
#include <faust/dsp/dsp.h>
#include <faust/dsp/llvm-dsp.h>


typedef llvm_dsp_factory W_Factory;
typedef llvm_dsp W_Dsp;

W_Factory *w_createDSPFactoryFromFile(const char *filepath, const char *dsp_libs_path, char *err_msg_c);

void w_deleteDSPFactory(W_Factory* factory);

W_Dsp *w_createDSPInstance(W_Factory* factory, int sample_rate);

struct W_DspInfo {
    int num_inputs;
    int num_outputs;
};

W_DspInfo w_getDSPInfo(W_Dsp *dsp);

void w_computeDSP(W_Dsp *dsp, int count, float **buf);

void w_deleteDSPInstance(W_Dsp *dsp);

struct W_MidiHandler;

W_MidiHandler *w_buildMidiUI(W_Dsp *dsp);

void w_deleteMidiHandler(W_MidiHandler *h);

void w_handleData1(W_MidiHandler *h, double time, int type, int channel, int data1);

void w_handleData2(W_MidiHandler *h, double time, int type, int channel, int data1, int data2);
