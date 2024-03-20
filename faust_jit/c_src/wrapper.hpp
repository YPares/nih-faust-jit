// Bindings will be generated by bindgen for each definition present in this
// file and everything it includes (recursively). It is therefore kept minimal
// and as simple as possible. This is why we re-declare here opaquely the few
// faust types we need, instead of including the faust headers directly.

struct llvm_dsp_poly_factory;
struct dsp_poly;

typedef llvm_dsp_poly_factory WFactory;
typedef dsp_poly WDsp;

WFactory *w_createDSPFactoryFromFile(const char *filepath, const char *dsp_libs_path, char *err_msg_c);

void w_deleteDSPFactory(WFactory *factory);

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
WDsp *w_createDSPInstance(WFactory *factory, int sample_rate, int nvoices, bool group_voices);

struct WDspInfo
{
    int num_inputs;
    int num_outputs;
};

WDspInfo w_getDSPInfo(WDsp *dsp);

void w_computeBuffer(WDsp *dsp, int count, float **buf);

void w_deleteDSPInstance(WDsp *dsp);

enum WWidgetDeclType
{
    TAB_BOX = 0,
    HORIZONTAL_BOX,
    VERTICAL_BOX,
    CLOSE_BOX,
    BUTTON,
    CHECK_BUTTON,
    HORIZONTAL_SLIDER,
    VERTICAL_SLIDER,
    NUM_ENTRY,
    HORIZONTAL_BARGRAPH,
    VERTICAL_BARGRAPH,
};

// The label is not part of WWidgetDecl because it may not outlive a call to a
// WWidgetDeclCallback
struct WWidgetDecl
{
    WWidgetDeclType typ;
    float *zone;
    float init;
    float min;
    float max;
    float step;
};

typedef void (*WWidgetDeclCallback)(void *context, const char *label, WWidgetDecl decl);

struct WUIs;

WUIs *w_createUIs(WDsp *dsp, WWidgetDeclCallback callback, void *gui_builder);

void w_deleteUIs(WUIs *h);

void w_handleMidiEvent(WUIs *h, double time, const unsigned char bytes[3]);