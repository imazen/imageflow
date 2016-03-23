module Imageflow
  class Job
    def initialize(context: )
      @c = context
      @ptr =  @c.call_method(:job_create)
      @keepalive = []
      record_nothing #Don't record anything by default
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

    def record_nothing
      configure_recording  record_graph_versions: false,
                           record_frame_images: false,
                           render_graph_versions: false,
                           render_animated_graph: false,
                           render_last_graph: false


    end

    def debug_record_gif
      configure_recording  record_graph_versions: true,
                           record_frame_images: true,
                           render_graph_versions: true,
                           render_animated_graph: true,
                           render_last_graph: true


    end

    def configure_recording(record_graph_versions:, record_frame_images:, render_last_graph:, render_graph_versions:,  render_animated_graph: )
      @c.call_method(:job_configure_recording, @ptr, record_graph_versions, record_frame_images, render_last_graph, render_graph_versions, render_animated_graph)
    end

    def get_buffer_bytes(resource_id:)
      buffer = get_buffer(resource_id: resource_id)
      raise "Buffer pointer null" if buffer[:buffer] == FFI::Pointer.new(0)
      buffer[:buffer].get_bytes(0, buffer[:buffer_size])
    end

    def insert_resources(graph: )
      @c.call_method(:job_insert_resources_into_graph, @ptr, graph.ptr_ptr_graph)
    end

    def execute (graph: )
      @c.call_method(:job_execute, @ptr, graph.ptr_ptr_graph)
    end

    def get_input_resource_info(placeholder_id:  )
      info = Imageflow::Native::FlowJobInputResourceInfo.new

      @c.call_method(:job_get_input_resource_info_by_placeholder_id, @ptr, placeholder_id.to_i, info )

      info
    end


  end
end