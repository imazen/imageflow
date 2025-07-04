# frozen_string_literal: true

require 'ffi'

# ImageflowFFI is the low-level module that interacts directly with the
# native Imageflow library via the Foreign Function Interface (FFI).
module ImageflowFFI
  extend FFI::Library

  # Determine the correct native library file based on OS
  lib_name = FFI::Platform.mac? ? 'libimageflow.dylib' : 'libimageflow.so'
  # The library is expected to be in the same directory as this file.
  lib_path = File.expand_path(File.join(__dir__, lib_name))

  # Check if the library exists before trying to load it.
  unless File.exist?(lib_path)
    raise LoadError, "Could not find Imageflow native library at #{lib_path}. " \
                     "Please ensure it has been built and copied to the correct location."
  end

  ffi_lib lib_path

  # Attach the ABI version functions.
  # These functions are part of the stable ABI.
  attach_function :imageflow_abi_version_major, [], :long
  attach_function :imageflow_abi_version_minor, [], :long
end
