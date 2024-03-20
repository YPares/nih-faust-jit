import("stdfaust.lib");

f = hslider("h:midi/freq", 220, 1, 20000, 0.1);
gate = button("h:midi/gate");
gain = hslider("h:midi/gain", 1, 0, 1, 0.001);

sub_amt = hslider("h:osc/h:sub/amount", 0, 0, 2, 0.001);
sub_ratio = hslider("h:osc/h:sub/ratio", 2, 2, 20, 1);
detune = hslider("h:osc/detune", 0, 0, 12, 0.01) : ba.semi2ratio;

lg = hslider("h:mix/lgain", 0.5, 0, 1, 0.01);
rg = hslider("h:mix/rgain", 0.5, 0, 1, 0.01);
noise_amt = hslider("h:noise/amount", 0, 0, 10, 0.001);
noise_decay = hslider("h:noise/decay", 0.5, 0, 1, 0.001);
noise_env = gate : en.adsr(0.01, noise_decay, 0, 0);

f_smoo = f;// : si.smoo;
gate_smoo = gate : si.smoo;

stereo_vu = hgroup("VU", par(i,2, vbargraph("",-1,1)));

process = (os.sawtooth(f_smoo)
            + os.sawtooth(f_smoo/(detune*detune))
            + os.sawtooth(f_smoo/detune)
            + os.sawtooth(f_smoo*detune)
            + os.sawtooth(f_smoo*detune*detune)
            + os.osc(f_smoo/sub_ratio) * sub_amt
            + no.pink_noise * noise_env * noise_amt
          ) * gate_smoo * gain
        <: _*lg,_*rg
        : stereo_vu;
