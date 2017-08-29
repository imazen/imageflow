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

        format = :jpeg if output_path =~ /\.jpe?g$/i
        format = :png if output_path =~ /\.png$/i

        command_string = "?w=#{opts.width}&h=#{opts.height}&mod=#{opts.crop_image ? 'crop': 'max'}&format=#{format}&decoder.min_precise_scaling_ratio=#{opts.stop_block_scaling_at || 2.1}&down.colorspace=#{opts.linear ? :linear : :srgb}"


        job.execute framewise: {steps: [
            {command_string: {
                kind: "ir4",
                decode: 0,
                encode: 1,
                value: command_string
            }}
        ]}
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
