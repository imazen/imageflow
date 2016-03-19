module Imageflow
  class Job
    def initialize(context: )
      @c = context
      @ptr =  @c.call_method(:job_create)
      @keepalive = []
    end


    def destroy!
      @c.call_method(:job_destroy, @ptr)
      @ptr = nil
    end
    def destroyed?
      @ptr.nil? || @ptr.null?
    end

    def add_input_buffer(placeholder_id:, bytes:)
      buffer = FFI::MemoryPointer.new(:char, bytes.bytesize) # Allocate memory sized to the data
      buffer.put_bytes(0, bytes)
      @keepalive << buffer
      @c.call_method(:job_add_buffer, @ptr, :flow_input, placeholder_id, buffer, bytes.bytesize, false)
    end

    def add_output_buffer(placeholder_id:)
      @c.call_method(:job_add_buffer, @ptr, :flow_output, placeholder_id, nil, 0, true)
    end

    def get_buffer(resource_id:)
      Native::FlowJobResourceBuffer.new @c.call_method(:job_get_buffer, @ptr, resource_id)
    end

    def get_buffer_bytes(resource_id:)
      buffer = get_buffer(resource_id: resource_id)
      buffer.buffer.get_bytes(0, buffer.buffer_size)
    end


    def execute (graph: )
      @c.call_method(:job_insert_resources_into_graph, @ptr, graph.ptr_ptr_graph)
      @c.call_method(:job_execute, @ptr, graph.ptr_ptr_graph)
    end


  end
end