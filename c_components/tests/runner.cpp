#define CATCH_CONFIG_RUNNER
#include "catch.hpp"

extern "C" int run_c_components_tests() {
    return Catch::Session().run( 0, NULL );
}

extern "C" int run_c_components_test_failure() {
    char * argv[] = {".fail"};
    return Catch::Session().run(1, argv );
}
TEST_CASE("Test failure works 2", ".fail")
{
    REQUIRE(false);
}
