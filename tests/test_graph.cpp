#include <glenn/png/png.h>
#include "catch.hpp"
#include "unistd.h"
#include <sys/types.h>
#include <sys/stat.h>
#include <errno.h>
#include <stdio.h>

#include "imageflow.h"

#include "fastscaling_private.h"
#include "weighting_test_helpers.h"
#include "trim_whitespace.h"
#include "string.h"
#include "lcms2.h"
#include "png.h"
#include "curl/curl.h"
#include "curl/easy.h"

TEST_CASE ("create tiny graph", "")
{
    Context * c = Context_create();
    flow_graph * g = flow_graph_create(c, 10, 10, 200);
    int32_t last;
    if (g == nullptr) goto cleanup;

    last = flow_node_create_canvas(c, g, -1, Bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, g, last, 300, 200);

    REQUIRE(g->edges[0].from == 0);
    REQUIRE(g->edges[0].to == 1);
    REQUIRE(g->edge_count == 1);
    REQUIRE(g->node_count == 2);

    cleanup:

        if (g != nullptr){
            flow_graph_destroy(c, g);
            g = nullptr;
        }

        if (c != nullptr) {
            Context_destroy(c);
            c = nullptr;
        }

}