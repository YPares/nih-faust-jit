declare options "[midi:on][nvoices:1]";

import("stdfaust.lib");

f = hslider("freq",220,50,10000,0.1);
v = hslider("gain",0.1,0,1,0.01);
t = button("gate") : si.smoo;

process = os.osc(f)*v*t , os.osc(f)*v*t;
