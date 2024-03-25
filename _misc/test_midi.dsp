declare options "[midi:on]";

import("stdfaust.lib");

f = hslider("v:midi/freq",220,20,10000,0.1);
v = hslider("v:midi/gain",0.1,0,1,0.01);
t = button("v:midi/gate") : si.smoo;

cc = hslider("v:global/CC74[midi:ctrl 74]",0,0,150,0.1);
tp = checkbox("v:global/Transport[midi:start][midi:stop]");
clk = checkbox("v:global/Clock[midi:clock]") : front : freq : hbargraph("ClockFreq",0,100);

// detect front
front(x) = (x-x') != 0.0;

// count number of peaks during one second
freq(x) = (x-x@ma.SR) : + ~ _;

process = os.osc(f)*v*t,tp : attach : _,cc : attach : _,clk : attach <: _,_;
