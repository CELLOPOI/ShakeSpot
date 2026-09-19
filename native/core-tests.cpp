#include "core.h"
#include <iostream>
#include <vector>
#include <functional>
#include <stdexcept>
using namespace shakespot;
namespace {
void require(bool value) { if (!value) throw std::runtime_error("Assertion failed."); }
using Path = std::function<std::pair<double, double>(double)>;
std::vector<double> trace(Path path, int count = 80, double step = 16, int sensitivity = 3, bool down = false) {
    Detector detector; detector.sensitivity(sensitivity); std::vector<double> result;
    for (int i = 0; i < count; ++i) { double t = i * step; auto [x,y] = path(t); if (detector.add(t,x,y,down)) result.push_back(t); }
    return result;
}
auto wave = [](double t) { return std::pair{85 * std::sin(t / 48), 0.0}; };
}
int main() {
    const std::pair<const char*, std::function<void()>> tests[]{
        {"Horizontal shake", []{ require(!trace(wave).empty()); }},
        {"Vertical shake", []{ require(!trace([](double t){ return std::pair{0.0,85*std::sin(t/48)}; }).empty()); }},
        {"Diagonal shake", []{ require(!trace([](double t){ return std::pair{85*std::sin(t/48),60*std::sin(t/48)}; }).empty()); }},
        {"Ordinary movement", []{ require(trace([](double t){ return std::pair{t*.18,35*std::sin(t/400)}; }).empty()); }},
        {"Fast one-way movement", []{ require(trace([](double t){ return std::pair{t*5,t*2}; }).empty()); }},
        {"Tiny jitter", []{ require(trace([](double t){ return std::pair{4*std::sin(t/12),5*std::cos(t/9)}; }).empty()); }},
        {"Slow back and forth", []{ require(trace([](double t){ return std::pair{90*std::sin(t/420),0.0}; },200).empty()); }},
        {"Single reversal", []{ require(trace([](double t){ return std::pair{t<250?t*2:(500-t)*2,0.0}; },32).empty()); }},
        {"Large sweep", []{ require(trace([](double t){ return std::pair{900*std::sin(t/70),0.0}; }).empty()); }},
        {"Drag suppression", []{ require(trace(wave,80,16,3,true).empty()); }},
        {"Repeated trigger cooldown", []{ auto result=trace(wave,160); require(result.size()>=3); for(size_t i=1;i<result.size();++i) require(result[i]-result[i-1]>=220); }},
        {"No stale retrigger", []{ Detector d; int count=0; double x=0; for(int i=0;i<60;++i){ x=wave(i*16).first; count+=d.add(i*16,x,0); } require(count>0); for(int i=60;i<160;++i) require(!d.add(i*16,x,0)); }},
        {"Fresh gesture after gap", []{ Detector d; int count=0; for(int j=0;j<2;++j) for(int i=0;i<60;++i) count+=d.add(j*2000+i*16,wave(i*16).first,0); require(count>=2); }},
        {"Release cooldown", []{ Detector d; d.add(0,0,0,true); for(int i=0;i<11;++i) require(!d.add(i*16+1,wave(i*16).first,0)); int count=0; for(int i=0;i<60;++i) count+=d.add(200+i*16,wave(i*16).first,0); require(count>0); }},
        {"Gap invalidates partial gesture", []{ Detector d; for(int j=0;j<2;++j) for(int i=0;i<10;++i) require(!d.add(j*1000+i*16,wave(i*16).first,0)); }},
        {"Sensitivity", []{ auto small=[](double t){ return std::pair{30*std::sin(t/40),0.0}; }; require(trace(small,80,16,1).empty()); require(!trace(small,80,16,5).empty()); }},
        {"Invalid values", []{ Detector d; require(!d.add(NAN,0,0)); require(!d.add(1,INFINITY,0)); require(!d.add(10,0,0)); require(!d.add(5,0,0)); }},
        {"Different sample rates", []{ for(int interval:{8,16,25,33}) require(!trace(wave,1000/interval,interval).empty()); }},
        {"Animation phases", []{ Animation a; a.trigger(0,4,1100); require(a.frame(75).scale>1&&a.frame(75).scale<4); require(a.frame(400).scale==4); require(a.frame(980).scale>1&&a.frame(980).scale<4); require(!a.frame(1100).visible); }},
        {"Smooth shrink retrigger", []{ Animation a; a.trigger(0,4,1000); auto before=a.frame(850); a.trigger(850,4,1000); require(a.frame(850).scale==before.scale); require(a.frame(1000).scale==4); require(a.frame(1600).visible); require(!a.frame(1850).visible); }},
        {"Smooth growth retrigger", []{ Animation a; a.trigger(0,6,500); auto before=a.frame(70); a.trigger(70,6,500); require(a.frame(70).scale==before.scale); require(a.frame(220).scale==6); }},
        {"Pause resets animation", []{ Animation a; a.trigger(0,4,1100); a.reset(); require(!a.frame(100).visible); }},
        {"Settings clamp and startup default", []{ Settings s; require(!s.startup); s.sensitivity=99;s.maximumScale=NAN;s.durationMs=-1;s.normalize();require(s.sensitivity==5&&s.maximumScale==4&&s.durationMs==500); }},
        {"Right edge mirrors", []{ require(flipAtEdge(1919,0,200,false,24)); require(!flipAtEdge(0,1919,200,true,24)); }},
        {"Edge hysteresis", []{ require(flipAtEdge(1700,210,200,true,24)); require(!flipAtEdge(1700,210,200,false,24)); require(!flipAtEdge(1600,300,200,true,24)); }}
    };
    int passed=0;
    for(const auto& [name,body]:tests) { try {body();++passed;std::cout<<"PASS "<<name<<'\n';} catch(const std::exception& e) {std::cerr<<"FAIL "<<name<<": "<<e.what()<<'\n';} }
    std::cout<<passed<<"/"<<std::size(tests)<<" passed\n";
    return passed==std::size(tests)?0:1;
}
