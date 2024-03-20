#include "wrapper.hpp"

#include <faust/dsp/libfaust.h>
#include <faust/dsp/dsp.h>
#include <faust/dsp/llvm-dsp.h>

#include <faust/dsp/poly-dsp.h>
#include <iostream>
#include <faust/dsp/poly-llvm-dsp.h>

#include <faust/midi/midi.h>
#include <faust/gui/MidiUI.h>

// These static vars must be declared in the application code. See
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

WDsp *w_createDSPInstance(WFactory *factory, int sample_rate, int nvoices, bool group_voices)
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

    WDsp *dsp = factory->createPolyDSPInstance(nvoices, midiControlledVoices, group_voices);
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
    dsp->instanceClear();
    delete dsp;
}

class WidgetDeclGUI : public GUI
{
private:
    WWidgetDeclCallback fCallback;
    void *fBuilder;

public:
    WidgetDeclGUI(WWidgetDeclCallback callback, void *builder) : GUI(), fCallback(callback), fBuilder(builder)
    {
    }

    ~WidgetDeclGUI()
    {
    }

    void openTabBox(const char *label)
    {
        fCallback(fBuilder, label, {TAB_BOX, nullptr, 0, 0, 0, 0});
    }

    void openHorizontalBox(const char *label)
    {
        fCallback(fBuilder, label, {HORIZONTAL_BOX, nullptr, 0, 0, 0, 0});
    }

    void openVerticalBox(const char *label)
    {
        fCallback(fBuilder, label, {VERTICAL_BOX, nullptr, 0, 0, 0, 0});
    }

    void closeBox()
    {
        fCallback(fBuilder, "", {CLOSE_BOX, nullptr, 0, 0, 0, 0});
    }

    void addButton(const char *label, FAUSTFLOAT *zone)
    {
        fCallback(fBuilder, label, {BUTTON, zone, 0, 0, 0, 0});
    }

    void addCheckButton(const char *label, FAUSTFLOAT *zone)
    {
        fCallback(fBuilder, label, {CHECK_BUTTON, zone, 0, 0, 0, 0});
    }

    void addVerticalSlider(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT init, FAUSTFLOAT min, FAUSTFLOAT max, FAUSTFLOAT step)
    {
        fCallback(fBuilder, label, {VERTICAL_SLIDER, zone, init, min, max, step});
    }

    void addHorizontalSlider(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT init, FAUSTFLOAT min, FAUSTFLOAT max, FAUSTFLOAT step)
    {
        fCallback(fBuilder, label, {HORIZONTAL_SLIDER, zone, init, min, max, step});
    }
    void addNumEntry(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT init, FAUSTFLOAT min, FAUSTFLOAT max, FAUSTFLOAT step)
    {
        fCallback(fBuilder, label, {NUM_ENTRY, zone, init, min, max, step});
    }

    void addHorizontalBargraph(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT min, FAUSTFLOAT max)
    {
        fCallback(fBuilder, label, {HORIZONTAL_BARGRAPH, zone, 0, min, max, 0});
    }

    void addVerticalBargraph(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT min, FAUSTFLOAT max)
    {
        fCallback(fBuilder, label, {VERTICAL_BARGRAPH, zone, 0, min, max, 0});
    }

    // -- soundfiles. TODO

    void addSoundfile(const char *label, const char *filename, Soundfile **sf_zone) {}

    // -- metadata declarations. Unused here

    void declare(FAUSTFLOAT *, const char *, const char *) {}
};

struct WUIs
{
    midi_handler *fMidiHandler;
    MidiUI *fMidiUI;
    WidgetDeclGUI *fWidgetDeclGUI;
};

WUIs *w_createUIs(WDsp *dsp, WWidgetDeclCallback callback, void *gui_builder)
{
    WUIs *uis = new WUIs();
    uis->fMidiHandler = new midi_handler();
    uis->fMidiUI = new MidiUI(uis->fMidiHandler);
    uis->fWidgetDeclGUI = new WidgetDeclGUI(callback, gui_builder);
    dsp->buildUserInterface(uis->fMidiUI);
    dsp->buildUserInterface(uis->fWidgetDeclGUI);
    uis->fMidiUI->run();
    uis->fWidgetDeclGUI->run();
    return uis;
}

void w_deleteUIs(WUIs *uis)
{
    uis->fMidiUI->stop();
    uis->fWidgetDeclGUI->stop();
    delete uis->fMidiUI;
    delete uis->fMidiHandler;
    delete uis->fWidgetDeclGUI;
    delete uis;
}

void w_handleMidiEvent(WUIs *uis, double time, const unsigned char bytes[3])
{
    // Faust expects status (type) bits _not_ to be shifted, so
    // we leave status bits in place and just set the other ones
    // to zero:
    uint8_t type = bytes[0] & 0b11110000;
    uint8_t channel = bytes[0] & 0b00001111;

    if (type == midi::MIDI_PROGRAM_CHANGE || type == midi::MIDI_AFTERTOUCH)
        uis->fMidiHandler->handleData1(time, type, channel, bytes[1]);
    else
        uis->fMidiHandler->handleData2(time, type, channel, bytes[1], bytes[2]);

    GUI::updateAllGuis();
}
