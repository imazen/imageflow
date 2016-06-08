from conans import ConanFile, CMake
import os
import shutil

class ImageFlowConan(ConanFile):
    name = "imageflow"
    version = "0.1.0"
    license = "AGPLv3"
    settings = "os", "compiler", "build_type", "arch"
    requires = "littlecms/2.7@lasote/stable", "libpng/1.6.21@lasote/stable", "libjpeg-turbo/1.4.2@imazen/testing" , "giflib/5.1.3@lasote/stable"
    options = {"shared": [True, False], "build_tests": [True, False], "profiling": [True, False], "coverage": [True, False]}
    generators = "cmake"
    default_options = "shared=False", "build_tests=False", "coverage=False", "profiling=False", "libjpeg-turbo:shared=False", "libpng:shared=False", \
   					  "zlib:shared=False", "libcurl:shared=False", "OpenSSL:shared=True", \
   					  "imageflow:shared=True"
    exports = "lib/*", "CMakeLists.txt", "imageflow.h", "imageflow_advanced.h"
    

    def config(self):
        if self.settings.os != "Windows":  #giflib must be shared on windows?
            self.options["giflib"].shared = False

        if self.options.build_tests or self.options.profiling:
            self.requires("libcurl/7.47.1@lasote/stable")
            if self.settings.os == "Macos":
                self.options["libcurl"].darwin_ssl = False
                self.options["libcurl"].custom_cacert = True

        if self.options.build_tests:
            self.requires("catch/1.3.0@TyRoXx/stable")
            if self.settings.os != "Windows":  # Not supported in windows
                self.requires("theft/0.2.0@lasote/stable")
                self.requires("electric-fence/2.2.0@lasote/stable") ##### SLOWS IT DOWN

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
        build_dir = os.path.join(self.conanfile_directory, "build")
        if not os.path.exists(build_dir):
            os.mkdir(build_dir)
        else:
            self.clean_cmake_cache(build_dir)
        os.chdir(build_dir)
        cmake = CMake(self.settings)
        cmake_settings = ""

        if self.options.coverage:
            cmake_settings += " -DCOVERAGE=ON"
        if self.options.build_tests:
            cmake_settings += " -DENABLE_TEST=ON"
        if self.options.profiling:
            cmake_settings += " -DSKIP_LIBRARY=ON -DENABLE_TEST=OFF -DENABLE_PROFILING=ON"
        
        cmake_settings += " -DBUILD_SHARED_LIBS=ON" if self.options.shared else " -DBUILD_SHARED_LIBS=OFF"

        cmake_command = 'cmake "%s" %s %s' % (self.conanfile_directory, cmake.command_line, cmake_settings)
        cmake_build_command = 'cmake --build . %s' % cmake.build_config
        self.output.warn(cmake_command)
        self.run(cmake_command)
        self.output.warn(cmake_build_command)
        self.run(cmake_build_command)
        if self.options.build_tests:
            self.run('ctest -V -C Release')
            
    def package(self):
        self.copy("imageflow.h", dst="include", src="", keep_path=False)
        self.copy("imageflow_advanced.h", dst="include", src="", keep_path=False)
        self.copy("*.h", dst="include", src="lib", keep_path=True)
        self.copy("*.so*", dst="lib", src="build/", keep_path=False)
        self.copy("*.a", dst="lib", src="build", keep_path=False)
        self.copy("*.lib", dst="lib", src="build", keep_path=False)
        self.copy("*.dll", dst="bin", src="build", keep_path=False)

    def package_info(self):
        self.cpp_info.libs = ['imageflow']
