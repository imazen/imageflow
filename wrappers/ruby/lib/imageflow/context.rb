module Imageflow
  class Context
    def self.release_auto_pointer(pointer)
      Native.context_destroy(pointer)
    end

    def initialize
      ptr = Native.context_create
      raise "Out of memory" if ptr.nil? || ptr.null?

      @c = FFI::AutoPointer.new(ptr, Context.method(:release_auto_pointer))
    end

    def destroy!
      Native.context_destroy(@c)
      @c.autorelease = false
      @c = nil
    end

    def is_destroyed?
      @c == nil
    end

    def raise_if_destroyed
      raise "Context is_destroyed; cannot be used." if is_destroyed?
    end

    def has_error?
      raise_if_destroyed
      Native.context_has_error(@c)
    end

    def raise_pending_error
      raise error_message if has_error?
    end


    def error_message
      raise_if_destroyed
      buffer = FFI::MemoryPointer.new(:char, 2048, true)

      Native.context_error_message(@c, buffer, 2048)

      buffer2 = FFI::MemoryPointer.new(:char, 2048, true)

      Native.context_stacktrace(@c, buffer2, 2048)

      "\n" + buffer.read_string + "\n" + buffer2.read_string
    end

    def stack_trace
      raise_if_destroyed
      buffer = FFI::MemoryPointer.new(:char, 2048, true)

      Native.context_stack_trace(@c, buffer, 2048)

      buffer.read_string
    end

    def create_graph (**args)
      Graph.new context: self, **args
    end

    def create_job (**args)
      Job.new context: self, **args
    end
    def call_method(name, *args)
      raise_if_destroyed
      raise_pending_error
      result = Native.send(name, @c, *args)
      raise_pending_error
      result
    end

  end
end