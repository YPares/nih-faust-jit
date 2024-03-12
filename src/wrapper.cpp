// This is an hpp file so that bindgen automatically includes C++ types (such as
// bool) used internally by these headers, even though they only export C
// functions.
#include "wrapper.hpp"

#include <faust/dsp/libfaust.h>
#include <faust/dsp/dsp.h>
#include <faust/dsp/llvm-dsp.h>

#include <faust/dsp/poly-dsp.h>
#include <iostream>
#include <faust/dsp/poly-llvm-dsp.h>

#include <faust/midi/midi.h>
#include <faust/gui/MidiUI.h>

// These global vars must be declared in the application code. See
// https://faustdoc.grame.fr/manual/architectures/#multi-controller-and-synchronization
std::list<GUI *> GUI::fGuiList;
ztimedmap GUI::gTimedZoneMap;

WFactory *w_createDSPFactoryFromFile(const char *filepath, const char *dsp_libs_path, char *err_msg_c)
{
    int argc = 3;
    const char *argv[] = {"--in-place", "-I", dsp_libs_path};
    std::string err_msg;
    WFactory *fac = createPolyDSPFactoryFromFile(filepath, argc, argv, "", err_msg, -1);
    strncpy(err_msg_c, err_msg.c_str(), 4096);
    return fac;
}

void w_deleteDSPFactory(WFactory *factory)
{
    delete factory;
}

WDsp *w_createDSPInstance(WFactory *factory, int sample_rate, int nvoices)
{
    // Whether the DSP voices should be controlled by faust from incoming MIDI
    // notes. If not, they will be all alive (and computed) all the time:
    bool midiControlledVoices = true;

    if (nvoices == -1)
    {
        // Get 'nvoices' from the metadata declaration. Should no longer be
        // necessary to do it here manually as from the next Faust release
        // (createPolyDSPInstance should do it itself).
        dsp *mono_dsp = factory->fProcessFactory->createDSPInstance();
        bool _midi_sync;
        MidiMeta::analyse(mono_dsp, _midi_sync, nvoices);
        delete mono_dsp;
    }

    if (nvoices == 0)
    {
        // nvoices was set to 0 at call-site OR it was not declared in the
        // script metadata => we consider the DSP to be a monophonic effect:
        nvoices = 1;
        midiControlledVoices = false;
    }

    WDsp *dsp = factory->createPolyDSPInstance(nvoices, midiControlledVoices, false);
    dsp->init(sample_rate);
    return dsp;
}

WDspInfo w_getDSPInfo(WDsp *dsp)
{
    return {dsp->getNumInputs(), dsp->getNumOutputs()};
}

void w_computeBuffer(WDsp *dsp, int count, float **buf)
{
    // We used --in-place when creating the DSP, so input and output should
    // be the same pointer
    dsp->compute(count, buf, buf);
}

void w_deleteDSPInstance(WDsp *dsp)
{
    delete dsp;
}

struct WMidiHandler
{
    midi_handler *handler;
    MidiUI *ui;
};

WMidiHandler *w_buildMidiHandler(WDsp *dsp)
{
    WMidiHandler *h = new WMidiHandler();
    h->handler = new midi_handler();
    h->ui = new MidiUI(h->handler);
    dsp->buildUserInterface(h->ui);
    h->ui->run();
    return h;
}

void w_deleteMidiHandler(WMidiHandler *h)
{
    h->ui->stop();
    delete h->ui;
    delete h->handler;
    delete h;
}

void w_handleMidiEvent(WMidiHandler *h, double time, const unsigned char bytes[3])
{
    // Faust expects status (type) bits _not_ to be shifted, so
    // we leave status bits in place and just set the other ones
    // to zero:
    uint8_t type = bytes[0] & 0b11110000;
    uint8_t channel = bytes[0] & 0b00001111;

    if (type == midi::MIDI_PROGRAM_CHANGE || type == midi::MIDI_AFTERTOUCH)
        h->handler->handleData1(time, type, channel, bytes[1]);
    else
        h->handler->handleData2(time, type, channel, bytes[1], bytes[2]);

    GUI::updateAllGuis();
}
