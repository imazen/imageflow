#include "catch.hpp"
#include "visual_helpers.h"

TEST_CASE("Test fill_rect", "")
{
flow_c* c = flow_context_create();

ERR(c);
flow_context_destroy(c);
}
