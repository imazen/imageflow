# frozen_string_literal: true

# Require the low-level FFI module
require_relative 'imageflow_ffi'

# Require all the generated data models
Dir[File.join(__dir__, 'imageflow', 'models', '*.rb')].each { |file| require file }

# Imageflow is the top-level module for the public Ruby API.
# It provides a clean, idiomatic interface to the Imageflow library.
module Imageflow
  class << self
    # Returns the version of the Imageflow ABI (Application Binary Interface)
    # as a 'major.minor' string.
    #
    # @return [String] The ABI version.
    def version
      "#{ImageflowFFI.imageflow_abi_version_major}.#{ImageflowFFI.imageflow_abi_version_minor}"
    end
  end
end
