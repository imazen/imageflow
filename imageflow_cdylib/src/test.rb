require 'ffi'

if RUBY_PLATFORM.include?('darwin')
  EXT = 'dylib'
else
  EXT = 'so'
end

module F
  extend FFI::Library
  ffi_lib 'target/release/libimageflowrs.' + EXT
  attach_function :imageflow_context_create, [], :pointer
  attach_function :imageflow_context_error_code, [:pointer], :int
end

c = F.imageflow_context_create
error = F.imageflow_context_error_code(c)
puts "Current error code is #{error}"