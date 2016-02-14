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

TEST_CASE ("execute graph", "")
{
    REQUIRE(true);

    Context * c = Context_create();

    //FrameGraph * g =



    cleanup:

    if (c != nullptr) {
        Context_destroy(c);
        c = nullptr;
    }

}