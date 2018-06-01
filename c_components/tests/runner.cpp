#define CATCH_CONFIG_RUNNER
#include "catch.hpp"

extern "C" int run_c_components_tests() {
    return Catch::Session().run();
}

extern "C" int run_c_components_test_failure() {
    Catch::Session session;
    Catch::ConfigData c;
    c.testsOrTags.push_back("[.]");
    session.useConfigData(c);
    return session.run();
}
TEST_CASE("Test failure works 2", "[.]")
{
    REQUIRE(false);
}
