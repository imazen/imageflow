module Imageflow
  module Vips
    class Cli
      def self.start(args)
        Cli.new(args).run!
      end

      def initialize(args)
        @args = args
      end


      def parse!(args)
        args = args.dup
        args << "-h" if args.empty?
        opts = ThumbnailOptions.new
        OptionParser.new do |opt|
          opt.banner = "Usage: imageflow-vips [options] image1.jpg image2.png image3.gif\n"

          opt.on('-s', '--size SIZE', 'shrink to SIZE or to WIDTHxHEIGHT') do |v|
            opts.thumbnail_size_str = v
            if v =~ /^\d+x\d+$/
              opts.width, opts.height = v.scan(/\d+/).map{|s| s.to_i}
            elsif v =~ /^\d+$/
              opts.width = opts.height = v.to_i
            else
              puts opt.help
              puts "unable to parse size #{v} -- use eg. 128 or 200x300\n"
              raise ParseError
            end
          end
          opt.on('-o', '--output FORMAT', 'set output to FORMAT') { |v| opts.output_format_string = v }
          opt.on('-f', '--format FORMAT', 'set output format string to FORMAT') { |v| opts.output_format_string = v }
          opt.on('-e', '--eprofile PROFILE', 'export with PROFILE') { |v| opts.export_profile = v }
          opt.on('-i', '--iprofile PROFILE', 'import untagged images with PROFILE') { |v| opts.import_profile = v }
          opt.on('-a', '--linear', 'process in linear space') { |v| opts.linear = v }
          opt.on('-c', '--crop', 'crop exactly to SIZE') { |v| opts.crop = v }
          opt.on('-t', '--rotate', 'auto-rotate based on EXIF data') { |v| opts.rotate_image = v }
          opt.on('-d', '--delete', 'delete profile from exported image') { |v| opts.delete_profile = v }
        end.parse!(args)
        opts.input_files = args.reject{|f| f.nil? || f.empty?}
        if args.empty?
          puts opt.help
          puts "You must specify at least one input image to create a thumbnail for\n"
          raise ParseError
        end
        opts
      end

      def run!
        opts = parse!(@args)

        exit_code = Engine.new(thumbnail_options: opts).run!
        exit_code
      end
    end
  end
end