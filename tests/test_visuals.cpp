#include "catch.hpp"
#include "helpers_visual.h"

TEST_CASE("Test fill_rect", "")
{
flow_c* c = flow_context_create();

ERR(c);
flow_context_destroy(c);
}
