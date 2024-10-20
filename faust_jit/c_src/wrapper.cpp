#include "wrapper.hpp"

#include <faust/dsp/libfaust.h>
#include <faust/dsp/dsp.h>
#include <faust/dsp/llvm-dsp.h>

#include <faust/dsp/poly-dsp.h>
#include <iostream>
#include <faust/dsp/poly-llvm-dsp.h>

#include <faust/dsp/timed-dsp.h>

#include <faust/midi/midi.h>
#include <faust/gui/MidiUI.h>

#ifdef DEFINE_FAUST_STATIC_VARS
// These static vars must be declared in the application code. See
// https://faustdoc.grame.fr/manual/architectures/#multi-controller-and-synchronization
std::list<GUI *> GUI::fGuiList;
ztimedmap GUI::gTimedZoneMap;
#endif

WFactory *w_createDSPFactoryFromFile(const char *filepath, const int argc, const char *argv[], char *err_msg_c)
{
    std::string err_msg;
    WFactory *fac = createPolyDSPFactoryFromFile(filepath, argc, argv, "", err_msg, -1);
    strncpy(err_msg_c, err_msg.c_str(), 4096);
    return fac;
}

void w_writeFactoryToFolder(WFactory *factory, const char *folder)
{
    auto prefix = std::string(folder) + "/code";
    writePolyDSPFactoryToMachineFile(factory, prefix, "");
}

WFactory *w_readFactoryFromFolder(const char *folder, char *err_msg_c)
{
    auto prefix = std::string(folder) + "/code";
    std::string err_msg;
    WFactory *fac = readPolyDSPFactoryFromMachineFile(prefix, "", err_msg);
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

    // timed_dsp is needed for sample-accurate control (such as for MIDI clock).
    // See https://faustdoc.grame.fr/manual/architectures/#sample-accurate-control
    WDsp *dsp = new timed_dsp(factory->createPolyDSPInstance(nvoices, midiControlledVoices, group_voices));
    dsp->init(sample_rate);
    return dsp;
}

WDsp *w_cloneDSPInstance(WDsp *dsp) {
    return dsp->clone();
}

DspInfo w_getDSPInfo(WDsp *dsp)
{
    return {dsp->getSampleRate(), dsp->getNumInputs(), dsp->getNumOutputs()};
}

void w_computeDSP(WDsp *dsp, int count, float **buf)
{
    // We used --in-place when creating the DSP, so input and output should
    // be the same pointer
    //
    // -1 means that MIDI events that were sent before (for this buffer) were
    // already timestamped using sample numbers
    dsp->compute(-1, count, buf, buf);
}

void w_deleteDSPInstance(WDsp *dsp)
{
    dsp->instanceClear();
    delete dsp;
}

extern "C"
{
    // These will be defined in Rust
    void rs_declare_widget(void *builder, const char *label, WWidgetDecl decl);
    void rs_declare_metadata(void *builder, FAUSTFLOAT *zone, const char *key, const char *value);
}

class WidgetDeclGUI : public GUI
{
private:
    void *fBuilder;

public:
    WidgetDeclGUI(void *builder) : GUI(), fBuilder(builder)
    {
    }

    ~WidgetDeclGUI()
    {
    }

    void openTabBox(const char *label)
    {
        rs_declare_widget(fBuilder, label, {TAB_BOX, nullptr, 0, 0, 0, 0});
    }

    void openHorizontalBox(const char *label)
    {
        rs_declare_widget(fBuilder, label, {HORIZONTAL_BOX, nullptr, 0, 0, 0, 0});
    }

    void openVerticalBox(const char *label)
    {
        rs_declare_widget(fBuilder, label, {VERTICAL_BOX, nullptr, 0, 0, 0, 0});
    }

    void closeBox()
    {
        rs_declare_widget(fBuilder, "", {CLOSE_BOX, nullptr, 0, 0, 0, 0});
    }

    void addButton(const char *label, FAUSTFLOAT *zone)
    {
        rs_declare_widget(fBuilder, label, {BUTTON, zone, 0, 0, 0, 0});
    }

    void addCheckButton(const char *label, FAUSTFLOAT *zone)
    {
        rs_declare_widget(fBuilder, label, {CHECK_BUTTON, zone, 0, 0, 0, 0});
    }

    void addVerticalSlider(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT init, FAUSTFLOAT min, FAUSTFLOAT max, FAUSTFLOAT step)
    {
        rs_declare_widget(fBuilder, label, {VERTICAL_SLIDER, zone, init, min, max, step});
    }

    void addHorizontalSlider(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT init, FAUSTFLOAT min, FAUSTFLOAT max, FAUSTFLOAT step)
    {
        rs_declare_widget(fBuilder, label, {HORIZONTAL_SLIDER, zone, init, min, max, step});
    }
    void addNumEntry(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT init, FAUSTFLOAT min, FAUSTFLOAT max, FAUSTFLOAT step)
    {
        rs_declare_widget(fBuilder, label, {NUM_ENTRY, zone, init, min, max, step});
    }

    void addHorizontalBargraph(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT min, FAUSTFLOAT max)
    {
        rs_declare_widget(fBuilder, label, {HORIZONTAL_BARGRAPH, zone, 0, min, max, 0});
    }

    void addVerticalBargraph(const char *label, FAUSTFLOAT *zone, FAUSTFLOAT min, FAUSTFLOAT max)
    {
        rs_declare_widget(fBuilder, label, {VERTICAL_BARGRAPH, zone, 0, min, max, 0});
    }

    // -- soundfiles. TODO
    void addSoundfile(const char *label, const char *filename, Soundfile **sf_zone) {}

    void declare(FAUSTFLOAT *zone, const char *key, const char *value)
    {
        rs_declare_metadata(fBuilder, zone, key, value);
    }
};

struct WUIs
{
    midi_handler *fMidiHandler;
    MidiUI *fMidiUi;
    WidgetDeclGUI *fWidgetGui;
};

WUIs *w_createUIs(WDsp *dsp, void *gui_builder)
{
    WUIs *uis = new WUIs();
    uis->fMidiHandler = new midi_handler();
    uis->fMidiUi = new MidiUI(uis->fMidiHandler);
    uis->fWidgetGui = new WidgetDeclGUI(gui_builder);
    dsp->buildUserInterface(uis->fMidiUi);
    dsp->buildUserInterface(uis->fWidgetGui);
    uis->fMidiUi->run();
    uis->fWidgetGui->run();
    return uis;
}

void w_deleteUIs(WUIs *uis)
{
    uis->fMidiUi->stop();
    uis->fWidgetGui->stop();
    delete uis->fMidiUi;
    delete uis->fMidiHandler;
    delete uis->fWidgetGui;
    delete uis;
}

void w_updateAllGuis()
{
    GUI::updateAllGuis();
}

void w_handleRawMidi(WUIs *uis, double time, const unsigned char bytes[3])
{
    // Faust expects status (type) bits _not_ to be shifted, so
    // we leave status bits in place and just set the other ones
    // to zero:
    uint8_t type = bytes[0] & 0b11110000;
    uint8_t channel = bytes[0] & 0b00001111;

    if (type == midi::MIDI_CLOCK || type == midi::MIDI_START ||
        type == midi::MIDI_CONT || type == midi::MIDI_STOP)
        uis->fMidiHandler->handleSync(time, type);
    else if (type == midi::MIDI_PROGRAM_CHANGE || type == midi::MIDI_AFTERTOUCH)
        uis->fMidiHandler->handleData1(time, type, channel, bytes[1]);
    else
        uis->fMidiHandler->handleData2(time, type, channel, bytes[1], bytes[2]);
}

void w_handleMidiSync(WUIs *uis, double time, WMidiSyncMsg status)
{
    uis->fMidiHandler->handleSync(time, status);
}
