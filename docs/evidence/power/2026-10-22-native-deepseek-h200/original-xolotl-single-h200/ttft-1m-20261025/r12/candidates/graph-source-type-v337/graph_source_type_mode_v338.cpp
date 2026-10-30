#include "aster_graph_source_type_v338.hpp"
#include <cstdio>
#include <cstdlib>
__attribute__((constructor)) static void aster_graph_type_mode_guard() {
    if(aster_graph_types::parse(std::getenv("ASTER_XQ_GRAPH_SOURCE_TYPES"))==aster_graph_types::Mode::invalid) {
        std::fputs("ASTER_XQ_GRAPH_SOURCE_TYPES must be 0 or 1\n",stderr);
        std::_Exit(2);
    }
}
