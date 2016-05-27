from conans import ConanFile, CMake
import os

class ImageFlowConan(ConanFile):
    settings = "os", "compiler", "build_type", "arch"
    requires = "littlecms/2.7@lasote/stable", "libpng/1.6.21@lasote/stable", "libjpeg-turbo/1.4.2@imazen/testing" , "giflib/5.1.3@tostercx/stable"
    options = {"build_tests": [True, False], "profiling": [True, False], "coverage": [True, False]}
    generators = "cmake"
    default_options = "build_tests=True", "coverage=False", "profiling=False", "libjpeg-turbo:shared=False", "libpng:shared=False", \
   					  "zlib:shared=False", "libcurl:shared=False", "OpenSSL:shared=True", \
   					  "imageflow:shared=True"

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

    def build(self):
        if not os.path.exists("./build"):
            os.mkdir("./build")
        os.chdir("./build")
        cmake = CMake(self.settings)
        cmake_settings = ""
        if self.options.coverage:
            cmake_settings += " -DCOVERAGE=ON"
        if self.options.build_tests:
            cmake_settings += " -DENABLE_TEST=ON"
        if self.options.profiling:
            cmake_settings += " -DSKIP_LIBRARY=ON -DENABLE_TEST=OFF -DENABLE_PROFILING=ON"

        cmake_command = 'cmake "%s" %s %s' % (self.conanfile_directory, cmake.command_line, cmake_settings)
        cmake_build_command = 'cmake --build . %s' % cmake.build_config
        self.output.warn(cmake_command)
        self.run(cmake_command)
        self.output.warn(cmake_build_command)
        self.run(cmake_build_command)
        if self.options.build_tests:
            self.run('ctest -V -C Release')

