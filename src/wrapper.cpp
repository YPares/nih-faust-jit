// This is an hpp file so that bindgen automatically includes C++ types (such as
// bool) used internally by these headers, even though they only export C
// functions.
#include <faust/dsp/libfaust.h>
#include <faust/dsp/llvm-dsp.h>

llvm_dsp_factory *w_createDSPFactoryFromFile(const char *filepath, const char *dsp_libs_path, char *err_msg_c)
{
    int argc = 3;
    const char *argv[] = {"--in-place", "-I", dsp_libs_path};
    std::string err_msg;
    llvm_dsp_factory* fac = createDSPFactoryFromFile(filepath, argc, argv, "", err_msg, -1);
    strncpy(err_msg_c, err_msg.c_str(), 4096);
    return fac;
}

void w_deleteDSPInstance(llvm_dsp* dsp) {
    delete dsp;
}
