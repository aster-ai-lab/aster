#include "aster_draft_cumulative.hpp"
#include <cassert>
#include <cstdio>
#include <limits>
#include <algorithm>
#include <array>
#include <utility>
#include <vector>
int main() {
    using aster_draft::Mode;
    int checks = 0;
    assert(aster_draft::parse(nullptr) == Mode::off); ++checks;
    for (const auto & row : std::vector<std::pair<const char *,Mode>>{
            {"0",Mode::off},{"0.1",Mode::c010},{"0.25",Mode::c025},{"0.5",Mode::c050}}) {
        assert(aster_draft::parse(row.first) == row.second); ++checks;
    }
    for (const char * raw : {"", "1", "0.10", "0.50", "true", " 0.1", "0.1 ", "NaN", "inf", "-0.1", "0.1junk"}) {
        assert(aster_draft::parse(raw) == Mode::invalid); ++checks;
    }
    const float nan = std::numeric_limits<float>::quiet_NaN();
    const float inf = std::numeric_limits<float>::infinity();
    struct Case { std::array<float,5> logits; std::array<int,3> selected; };
    const std::vector<Case> inputs{
        {{0,0,0,0,0},{3,2,1}}, {{10,10,10,10,10},{5,5,5}},
        {{-10,-10,-10,-10,-10},{0,0,0}}, {{10,-10,10,10,10},{1,1,1}},
        {{inf,inf,inf,inf,inf},{5,5,5}}, {{-inf,0,0,0,0},{0,0,0}},
        {{nan,0,0,0,0},{0,0,0}}, {{10,nan,0,0,0},{1,1,1}},
        {{1,2,3,4,5},{5,5,5}}};
    const Mode modes[] = {Mode::off,Mode::c010,Mode::c025,Mode::c050};
    for (int index=0; index<4; ++index) {
        const Mode mode=modes[index];
        for (const auto & row : inputs) for (int count=0; count<=5; ++count) {
            const int expected = mode==Mode::off ? count : std::min(count,row.selected[index-1]);
            const int actual=aster_draft::truncate(mode,row.logits.data(),count);
            assert(actual==expected && actual>=0 && actual<=count); ++checks;
        }
    }
    float half[5] = {0,0,0,0,0};
    assert(aster_draft::truncate(Mode::c050,half,5)==1); ++checks;
    assert(aster_draft::truncate(Mode::c025,half,5)==2); ++checks;
    assert(aster_draft::truncate(Mode::c010,half,5)==3); ++checks;
    assert(aster_draft::truncate(Mode::off,nullptr,5)==5); ++checks;
    assert(aster_draft::truncate(Mode::c010,nullptr,5)==0); ++checks;
    assert(aster_draft::truncate(Mode::c010,half,6)==0); ++checks;
    assert(aster_draft::truncate(Mode::c010,half,-1)==0); ++checks;
    assert(aster_draft::truncate(Mode::invalid,half,5)==0); ++checks;
    std::printf("PASS %d strict-mode, draft-prefix, nonfinite and boundary checks\n",checks);
}
