// This is an hpp file so that bindgen automatically includes C++ types (such as
// bool) used internally by these headers, even though they only export C
// functions.
#include "wrapper.hpp"

#include <faust/midi/midi.h>
#include <faust/gui/MidiUI.h>
#include <iostream>

// These global vars must be declared in the application code. See
// https://faustdoc.grame.fr/manual/architectures/#multi-controller-and-synchronization
std::list<GUI*> GUI::fGuiList;
ztimedmap GUI::gTimedZoneMap;


llvm_dsp_factory *w_createDSPFactoryFromFile(const char *filepath, const char *dsp_libs_path, char *err_msg_c)
{
    int argc = 3;
    const char *argv[] = {"--in-place", "-I", dsp_libs_path};
    std::string err_msg;
    llvm_dsp_factory *fac = createDSPFactoryFromFile(filepath, argc, argv, "", err_msg, -1);
    strncpy(err_msg_c, err_msg.c_str(), 4096);
    return fac;
}

void w_deleteDSPInstance(llvm_dsp *dsp)
{
    delete dsp;
}

struct W_MidiHandler
{
    midi_handler *midi_handler;
    MidiUI *midi_ui;
};

W_MidiHandler *w_buildMidiUI(llvm_dsp *dsp)
{
    W_MidiHandler *h = new W_MidiHandler();
    h->midi_handler = new midi_handler();
    h->midi_ui = new MidiUI(h->midi_handler);
    dsp->buildUserInterface(h->midi_ui);
    h->midi_ui->run();
    return h;
}

void w_deleteMidiHandler(W_MidiHandler *h)
{
    h->midi_ui->stop();
    delete h->midi_ui;
    delete h->midi_handler;
    delete h;
}

void w_handleData1(W_MidiHandler *h, double time, int type, int channel, int data1) {
    h->midi_handler->handleData1(time, type, channel, data1);
}

void w_handleData2(W_MidiHandler *h, double time, int type, int channel, int data1, int data2) {
    h->midi_handler->handleData2(time, type, channel, data1, data2);
}
