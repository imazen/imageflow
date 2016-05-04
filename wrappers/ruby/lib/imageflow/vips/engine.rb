module Imageflow
  module Vips
    class Engine
      def initialize(thumbnail_options:)
        @opts = thumbnail_options
      end
      attr_reader :opts

=begin
        @delete_profile = false
        @linear = false
        @crop_image = false
        @rotate_image = false
        @width = 128
        @height = 128
        @import_profile = @export_profile = nil
=end

      def generate!(input_path:, output_path:)
        c = Imageflow::Context.new
        job = c.create_job
        #job.debug_record_gif

        job.add_input_file(placeholder_id: 0, filename: input_path)
        job.add_output_file(placeholder_id: 1, filename: output_path)

        input_info = job.get_decoder_info placeholder_id: 0

        w = input_info[:frame0_width]
        h = input_info[:frame0_height]

        instructions = Imageflow::Riapi::Instructions.new
        instructions.width = opts.width
        instructions.height = opts.height
        #todo - autorotate!
        instructions.mode = opts.crop_image ? :crop : :max

        instructions.precise_scaling_ratio = opts.stop_block_scaling_at || 2.1

        instructions.format = :jpeg if output_path =~ /\.jpe?g$/i
        instructions.format = :png if output_path =~ /\.png$/i


        instructions.floatspace = opts.linear ? :linear : :srgb

        c.set_floatspace_linear! if instructions.floatspace == :linear
        c.set_floatspace_srgb! if instructions.floatspace == :srgb



        # instructions.

        #cpu_time = ::Benchmark.realtime do
        gb = Imageflow::Riapi::GraphBuilder.new context: c
        g = gb.build_graph(job: job, input_placeholder_id: 0, output_placeholder_id: 1, source_info: input_info, instructions: instructions)
        job.execute graph: g
        c.destroy!
        c = nil
        #end



        #puts "Real milliseconds (1 thread): %.4f \n" % (cpu_time * 1000.0)


        #TODO rescue and return an appropriate exit code

        return 0
      ensure
        c.destroy! unless c.nil?
      end

      def run!
        exit_code = 0
        opts.input_files.each do |input_filename|
          meaningful_bit = File.basename(input_filename, File.extname(input_filename))
          output_filename = opts.output_format_string.gsub(/%s/, meaningful_bit)
          result = generate! input_path: input_filename, output_path: output_filename
          exit_code = result unless result == 0
        end
        exit_code
      end

    end
  end
end
