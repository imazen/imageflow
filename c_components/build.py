# Not used much
# ignore this
from conan.packager import ConanMultiPackager
import platform

if __name__ == "__main__":
    builder = ConanMultiPackager()
    builder.add_common_builds(shared_option_name="imageflow_c:shared", pure_c=True)
    x64_builds = []
    for build in builder.builds:
        if build[0]["arch"] != "x86":
            x64_builds.append([build[0], build[1]])

    builder.builds = x64_builds
    builder.run()

