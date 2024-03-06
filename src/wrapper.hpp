// This is an hpp file so that bindgen automatically includes C++ types (such as
// bool) used internally by these headers, even though they only export C
// functions.
#include <faust/dsp/libfaust.h>
#include <faust/dsp/llvm-dsp.h>

llvm_dsp_factory *w_createDSPFactoryFromFile(const char *filepath, const char *dsp_libs_path, char *err_msg_c);
void w_deleteDSPInstance(llvm_dsp* dsp);
