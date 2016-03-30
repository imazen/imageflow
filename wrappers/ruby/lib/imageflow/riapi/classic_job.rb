module Imageflow
  module Riapi
    class ClassicJob

      def initialize(image_source:, instructions:, requested_info: [])
        @source = image_source
        @instructions = instructions
        @requested_info = requested_info

      end

      attr_accessor :instructions, :source, :requested_info

      attr_reader :result_info, :result_bytes

      def result_content_type
        result_info[:mime_type]
      end


      def acquire_result_info!
        raise "result_info already acquired" unless result_info.nil?
        acquire(info_only: true)
      end

      def acquire_bytes!
        raise "Bytes already acquired" unless @result_bytes.nil?
        acquire(info_only: false)

      end


      def get_source_info(job:, placeholder_id:)
        info = job.get_input_resource_info placeholder_id: placeholder_id

        {
            preferred_mime_type: info[:preferred_mime_type],
            preferred_extension: info[:preferred_extension],
            frame0_width: info[:frame0_width],
            frame0_height: info[:frame0_height],
            frame0_post_decode_format: info[:frame0_post_decode_format],
            codec_id: info[:codec_id]
        }

      end


      def acquire (info_only: false)
        c = Context.new

        source.load_bytes!

        #Dir.mkdir("./node_frames") unless Dir.exist? "./node_frames"

        #puts "Writing graphs to " + File.expand_path("./")

        job = c.create_job
        #job.debug_record_gif

        job.add_input_buffer(placeholder_id: 0, bytes: source.bytes)
        job.add_output_buffer(placeholder_id: 1)


        @result_info ||= {}

        @result_info[:source]= get_source_info(job: job, placeholder_id: 0)

        return if info_only

        gb = GraphBuilder.new context: c

        g = gb.build_graph(input_placeholder_id: 0, output_placeholder_id: 1, source_info: result_info[:source], instructions: instructions)

        @result_info[:mime_type] = gb.result_mime_type


        job.execute graph: g

        @result_bytes = job.get_buffer_bytes(placeholder_id: 1)
      ensure
        c.destroy! unless c.nil?
      end

      private :acquire, :get_source_info


    end
  end
end
