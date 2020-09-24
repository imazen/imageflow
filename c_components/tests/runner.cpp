#define CATCH_CONFIG_RUNNER
#include "catch.hpp"

extern "C" int run_c_components_tests() {
    Catch::Session session;
    Catch::ConfigData c;
    c.listTests = true;
    //c.showSuccessfulTests = true;
    session.useConfigData(c);
    session.run();
    c.listTests = false;
    session.useConfigData(c);
    return session.run();
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
