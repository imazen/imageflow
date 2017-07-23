from conans import ConanFile, CMake
import os
import shutil

class ImageFlowConan(ConanFile):
    name = "imageflow_c"
    version = "0.1.0"
    license = "AGPLv3"
    settings = "os", "compiler", "build_type", "arch", "target_cpu"
    requires = "littlecms/2.7@lasote/stable", "libpng/1.6.21@lasote/stable", "libjpeg-turbo/1.5.1@imazen/testing"  #, "giflib/5.1.3@lasote/stable"
    options = {"shared": [True, False]}
    generators = "cmake"
    default_options = "shared=False", "libjpeg-turbo:shared=False", "libpng:shared=False", \
   					  "zlib:shared=False", "imageflow_c:shared=True"
    exports = "lib/*", "tests/*", "CMakeLists.txt", "imageflow.h", "imageflow_advanced.h"

#"libcurl:with_openssl=False"
    def config(self):
        if self.settings.os != "Windows":  # giflib/littlecms must be shared on windows?
            #self.options["giflib"].shared = False
            self.options["littlecms"].shared = False

        if self.scope.build_tests:
            self.requires("catch/1.3.0@TyRoXx/stable", dev=True)
            if self.settings.os != "Windows":  # Not supported in windows
                self.requires("theft/0.2.0@lasote/stable", dev=True)

    def imports(self):
        self.copy("*.so", dst="bin", src="bin")  # From bin to bin
        self.copy("*.dll", dst="bin", src="bin")  # From bin to bin
        self.copy("*.dylib*", dst="bin", src="lib")  # From lib to bin
        self.copy("*cacert.pem", dst="bin")  # Allows use libcurl with https without problems - except on darwin
        self.copy("*cacert.pem", dst=".")  # Allows use libcurl with https without problems
        self.copy("*.a", dst=".") # Copy all static libs to use in cargo build.

    def clean_cmake_cache(self, build_dir):
        def on_build_dir(x):
            return os.path.join(build_dir, x)

        try:
            shutil.rmtree(on_build_dir("CMakeFiles"))
            os.remove(on_build_dir("CMakeCache.txt"))
            os.remove(on_build_dir("cmake_install.cmake"))
            os.remove(on_build_dir("Makefile"))
        except:
            pass


    def build(self):
        self.output.warn('build_tests=%s debug_build=%s coverage=%s profiling=%s shared=%s target_cpu=%s' % (self.scope.build_tests, self.scope.debug_build, self.scope.coverage, self.scope.profiling, self.options.shared, self.settings.target_cpu))
        build_dir = os.path.join(self.conanfile_directory, "build")
        if not os.path.exists(build_dir):
            os.mkdir(build_dir)
        else:
            self.clean_cmake_cache(build_dir)
        os.chdir(build_dir)
        cmake = CMake(self.settings)
        cmake_settings = ""

        if self.scope.dev and self.scope.coverage:
            cmake_settings += " -DCOVERAGE=ON"
        if self.scope.dev and self.scope.debug_build:
            cmake_settings += " -DDEBUG_BUILD=ON"
        if self.scope.dev and self.scope.build_tests:
            cmake_settings += " -DENABLE_TEST=ON"
        if self.scope.dev and self.scope.profiling:
            cmake_settings += " -DSKIP_LIBRARY=ON -DENABLE_TEST=OFF -DENABLE_PROFILING=ON"


        cmake_settings += " -DBUILD_SHARED_LIBS=ON" if self.options.shared else " -DBUILD_SHARED_LIBS=OFF"
        cmake_settings += " -DBUILD_SHARED_LIBS=ON" if self.options.shared else " -DBUILD_SHARED_LIBS=OFF"
        cmake_settings += " -DTARGET_CPU=%s" % (self.settings.target_cpu) if self.settings.target_cpu else ""

        cmake_command = 'cmake "%s" %s %s' % (self.conanfile_directory, cmake.command_line, cmake_settings)
        cmake_build_command = 'cmake --build . %s' % cmake.build_config
        cmake_valgrind = "-D ExperimentalMemCheck" if self.scope.valgrind else ""

        cmake_test_command = 'ctest -V -C Release %s' % cmake_valgrind
        self.output.warn(cmake_command)
        self.run(cmake_command)
        self.output.warn(cmake_build_command)
        self.run(cmake_build_command)

        if self.scope.dev and self.scope.build_tests:
            if self.scope.skip_test_run:
                self.output.warn("Skipping tests; skip_test_run=False (perhaps for later valgrind use?)")
                self.output.warn("Would have run %s" % cmake_test_command)
            else:
                self.output.warn(cmake_test_command)
                self.run(cmake_test_command)
        else:
            self.output.warn("Skipping tests; build_tests=False")

    def package(self):
        self.copy("imageflow.h", dst="include", src="", keep_path=False)
        self.copy("imageflow_advanced.h", dst="include", src="", keep_path=False)
        self.copy("*.h", dst="include", src="lib", keep_path=True)
        self.copy("*.so*", dst="lib", src="build/", keep_path=False)
        self.copy("*.a", dst="lib", src="build", keep_path=False)
        self.copy("*.lib", dst="lib", src="build", keep_path=False)
        self.copy("*.dll", dst="bin", src="build", keep_path=False)

    def package_info(self):
        self.cpp_info.libs = ['imageflow_c']
