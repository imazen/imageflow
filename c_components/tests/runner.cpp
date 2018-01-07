#define CATCH_CONFIG_RUNNER
#include "catch.hpp"

extern "C" int run_c_components_tests() {
    return Catch::Session().run( 0, NULL );
}
