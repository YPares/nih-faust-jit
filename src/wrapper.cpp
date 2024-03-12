// This is an hpp file so that bindgen automatically includes C++ types (such as
// bool) used internally by these headers, even though they only export C
// functions.
#include "wrapper.hpp"

#include <faust/midi/midi.h>
#include <faust/gui/MidiUI.h>

// These global vars must be declared in the application code. See
// https://faustdoc.grame.fr/manual/architectures/#multi-controller-and-synchronization
std::list<GUI *> GUI::fGuiList;
ztimedmap GUI::gTimedZoneMap;

W_Factory *w_createDSPFactoryFromFile(const char *filepath, const char *dsp_libs_path, char *err_msg_c)
{
    int argc = 3;
    const char *argv[] = {"--in-place", "-I", dsp_libs_path};
    std::string err_msg;
    W_Factory *fac = createPolyDSPFactoryFromFile(filepath, argc, argv, "", err_msg, -1);
    strncpy(err_msg_c, err_msg.c_str(), 4096);
    return fac;
}

void w_deleteDSPFactory(W_Factory *factory)
{
    delete factory;
}

W_Dsp *w_createDSPInstance(W_Factory *factory, int sample_rate, int nvoices)
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

    W_Dsp *dsp = factory->createPolyDSPInstance(nvoices, midiControlledVoices, false);
    dsp->init(sample_rate);
    return dsp;
}

W_DspInfo w_getDSPInfo(W_Dsp *dsp)
{
    return {dsp->getNumInputs(), dsp->getNumOutputs()};
}

void w_computeBuffer(W_Dsp *dsp, int count, float **buf)
{
    // We used --in-place when creating the DSP, so input and output should
    // be the same pointer
    dsp->compute(count, buf, buf);
}

void w_deleteDSPInstance(W_Dsp *dsp)
{
    delete dsp;
}

struct W_MidiHandler
{
    midi_handler *midi_handler;
    MidiUI *midi_ui;
};

W_MidiHandler *w_buildMidiHandler(W_Dsp *dsp)
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

void w_handleMidiEvent(W_MidiHandler *h, double time, const uint8_t bytes[3])
{
    // Faust expects status (type) bits _not_ to be shifted, so
    // we leave status bits in place and just set the other ones
    // to zero:
    uint8_t type = bytes[0] & 0b11110000;
    uint8_t channel = bytes[0] & 0b00001111;

    if (type == midi::MIDI_PROGRAM_CHANGE || type == midi::MIDI_AFTERTOUCH)
        h->midi_handler->handleData1(time, type, channel, bytes[1]);
    else
        h->midi_handler->handleData2(time, type, channel, bytes[1], bytes[2]);

    GUI::updateAllGuis();
}
