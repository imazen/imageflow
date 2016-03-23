from conans import ConanFile, CMake

class ImageFlowConan(ConanFile):
    settings = "os", "compiler", "build_type", "arch"
    requires = "littlecms/2.7@lasote/stable", "libpng/1.6.21@lasote/stable", "libjpeg-turbo/1.4.2@lasote/stable" 
    options = {"build_tests": [True, False]}
    generators = "cmake"
    default_options = "build_tests=False", "libjpeg-turbo:shared=False", "libpng:shared=False", \
   					  "zlib:shared=False", "libcurl:shared=False", "OpenSSL:shared=True", \
   					  "imageflow:shared=True"

    def config(self):
        if self.options.build_tests:
            self.requires("catch/1.3.0@TyRoXx/stable")
            self.requires("libcurl/7.47.1@lasote/stable")
            if self.settings.os != "Windows":  # Not supported in windows
                self.requires("theft/0.2.0@lasote/stable")

    def imports(self):
        self.copy("*.so", dst="bin", src="bin")  # From bin to bin
        self.copy("*.dll", dst="bin", src="bin")  # From bin to bin
        self.copy("*.dylib*", dst="bin", src="lib")  # From lib to bin
        self.copy("*cacert.pem", dst="bin")  # Allows use libcurl with https without problems 
        self.copy("*cacert.pem", dst=".")  # Allows use libcurl with https without problems
