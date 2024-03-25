declare options "[midi:on]";

import("stdfaust.lib");

f = hslider("v:midi/freq[unit:Hz]",220,20,10000,0.1);
v = hslider("v:midi/gain[hidden:1]",0.1,0,1,0.01);
t = button("v:midi/gate[tooltip:The gate button]") : si.smoo;

cc = hslider("v:global/CC74[midi:ctrl 74][tooltip:Mapped to CC74]",0,0,150,0.1);
tp = checkbox("v:global/Transport[midi:start][midi:stop][tooltip:If Play is on or off]");
clk = checkbox("v:global/Clock[midi:clock][hidden:1]") : front : freq :
      hbargraph("ClockFreq[tooltip:Frequency of the received clock][unit:ppqn]",0,100);

// detect front
front(x) = (x-x') != 0.0;

// count number of peaks during one second
freq(x) = (x-x@ma.SR) : + ~ _;

process = os.osc(f)*v*t,tp : attach : _,cc : attach : _,clk : attach <: _,_;
