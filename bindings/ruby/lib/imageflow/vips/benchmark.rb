require 'thread'
require 'benchmark'
require 'imageflow'
require 'imageflow/vips'
require 'digest/sha1'

module Imageflow
  module Vips

    class BaseBenchmarkee
      def initialize(input_path: , config:, benchmark:, input_width:, input_height:, input_index: )
        @path = input_path
        @config = config
        @b = benchmark
        @w = input_width
        @h = input_height
        @output_w = config[:w]
        @output_h = (@output_w.to_f * @h.to_f / @w.to_f).to_i + 1
        @linear = config[:linear]
        @output_format = config[:format] || :png
        basename = File.basename(path, File.extname(path))
        hash = Digest::SHA1.hexdigest(config.inspect)[0..8]
        @output_path = File.join(b.dir, "#{shortname}_#{input_index}_#{basename}_#{output_w}__#{hash}.#{output_format}")
      end

      attr_reader :path, :config, :b, :w, :h, :output_w, :output_h, :linear, :output_path, :output_format

      def name
        self.class.to_s.downcase.gsub(/^.*::/, '')
      end

      def run!
        out_msg = `#{to_s}`
        result = $?.exitstatus
        raise "Command failed with code #{result}: #{to_s}\n" unless result == 0
        out_msg
      end
    end

    class VipsThumbnail < BaseBenchmarkee
      def shortname
        "vips"
      end

      def exe_name
        "vipsthumbnail"
      end

      def to_s
        lin = linear ? "--linear " : ""
        "#{exe_name} #{lin} --output=#{output_path} --size=#{output_w}x#{output_h} #{path}" #todo - add color profile
      end
    end
    class ImageflowVips < VipsThumbnail
      def shortname
        "flow"
      end

      def exe_name
        "imageflow-vips"
      end

      def run!
        args = to_s.split(/\s+/)
        args.shift #drop the exe name

        #we can't do anything about stdout/stderr :( OptionParser ties our hands
        result = Imageflow::Vips::Cli.start(args)
        $stdout.flush if $stdout.respond_to?(:flush)


        if result != 0
          raise "Failed with exit code #{result}: #{exe_name} #{args}"
        end

        ""
      end
    end

    class ImageflowVipsNoBlockscale < ImageflowVips
      def shortname
        "flowhq"
      end
      def to_s
        "#{super} --stop_block_scaling_at=300"
      end
    end




    class ImageMagick < BaseBenchmarkee
      def shortname
        "magick"
      end
      def exe_name
        "convert"
      end

      def to_s
        final_scale_factor = 2
        if linear
          "#{exe_name}  #{path} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize #{output_w}x#{output_h} -colorspace sRGB #{output_path}"
        else
          "#{exe_name} -define jpeg:size=#{output_w * final_scale_factor}x#{output_h * final_scale_factor} #{path} -set colorspace sRGB -filter Robidoux -resize #{output_w}x#{output_h} -colorspace sRGB #{output_path}"
        end
      end
    end
    class Reference < ImageMagick
      def shortname
        "reference"
      end
      def to_s
        if linear
          "#{exe_name} #{path} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize  #{output_w}x#{output_h} -colorspace sRGB #{output_path}"
        else
          "#{exe_name} #{path} -set colorspace sRGB -filter Robidoux -resize #{output_w}x#{output_h} -colorspace sRGB #{output_path}"
        end

      end
    end
    class IdealReference < ImageMagick
      def shortname
        "ideal_reference"
      end
      def to_s
        if linear
          "#{exe_name} #{path} -set colorspace sRGB -colorspace RGB -filter Mitchell -distort Resize #{output_w}x#{output_h} -colorspace sRGB #{output_path}"
        else
          "#{exe_name} #{path} -set colorspace sRGB  -filter Mitchell -distort Resize #{output_w}x#{output_h} -colorspace sRGB #{output_path}"
        end

      end
    end

    class Benchmark

      def initialize(output_stream: STDOUT, configs: nil, image_urls: nil, benchmarkees: nil)
        @log = output_stream
        @dir = File.expand_path("temp_vips_benchmarking")
        @image_urls = image_urls || ["gamma_dalai_lama_gray.jpg", "u1.jpg", "u6.jpg", "u1.jpg","u1.jpg","u1.jpg","u1.jpg","u1.jpg","u1.jpg", "mountain_800.jpg"].map do |name|
          "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/#{name}"
        end
        @images = []
        @op_configs = configs || [{linear: true, w: 128}, {linear: true, w: 400,}, {linear: true, w: 800 }, {linear: true, w: 1600}]
        #@op_configs = @op_configs + @op_configs.map{|h| h.dup.merge({linear: false})}

        @benchmarkees = benchmarkees || [ImageflowVips, VipsThumbnail, ImageMagick, Reference]


        @cpu_count = get_cpu_count
      end

      attr_accessor :log, :dir, :versions, :vips_available, :magick_available, :cpu_count, :images, :benchmarkees, :op_configs



      def benchmark!(log:)
        #We do configs one at a time
        @op_configs.each do |config|
          log << "\n\nBeginning configuration #{config.inspect}\n\n"
          #Create the permutations
          runners = benchmarkees.map  do |cls|
            images.each_with_index.map do |img, index|
              cls.new(input_path: img[:filename], input_index: index, input_width: img[:width], input_height: img[:height], config: config, benchmark: self)
            end
          end

          thread_count = [cpu_count.to_i - 1, images.length].min
          log << "Commands used for #{images.length} images (distributed among #{thread_count} threads/processes): \n\n"
          log << runners.map{|list| list.join("\n")}.join("\n")
          log << "\n\n"

          $stdout = StringIO.new
          $stderr = StringIO.new

          ::Benchmark.bm(80) do |bench|
            runners.each do |operations|


              results = []
              queue = Queue.new
              operations.each do |op|
                queue << op
              end

              bench.report(operations.first.name) do
                threads = []
                thread_count.times do |_|
                  threads << Thread.new do
                    # loop until there are no more things to do
                    until queue.empty?
                      # pop with the non-blocking flag set, this raises
                      # an exception if the queue is empty, in which case
                      # work_unit will be set to nil
                      work_unit = queue.pop(true) rescue nil
                      if work_unit
                        results << work_unit.run!
                      end
                    end
                    # when there is no more work, the thread will stop
                  end
                end

                # wait until all threads have completed processing
                threads.each { |t| t.join }
              end

              results.each do |r|
                log << r
              end

            end
          end
          $stdout.flush
          $stderr.flush

          log << $stdout.string
          log << $stderr.string
          $stdout = STDOUT
          $stderr = STDERR

          by_image = runners.transpose

          by_image.each do |set|

            png_images = set.map do |benchmarkee|
              output = benchmarkee.output_path
              unless output.end_with? ".png"
                new_output = output.gsub(/\.[a-zA-Z]+$/, ".png")
                out_msg = `convert #{output} -define png:compression-level=0 #{new_output}`
                log << out_msg unless $?.exitstatus == 0
                output = new_output
              end
              output
            end

            reference = png_images.pop

            log << "\nComparing against #{reference}\n"
            png_images.each do |output|
              log << `dssim #{reference} #{output}`
              log << "Visualize with > compare #{reference} #{output} -fuzz 1%  x:\n"
            end
          end
        end
      end

      def check_tools!

        @versions = ""
        @versions << `vipsthumbnail --vips-version`
        @vips_available = $?.exitstatus == 0

        @versions << "\n"

        imagemagick_ver = `convert --version`
        @versions << imagemagick_ver

        @magick_available = $?.exitstatus == 0
        if @magick_available
          unless imagemagick_ver =~ /HDRI/ && imagemagick_ver =~ /Q16/
            @magick_available = false
            @versions << "You must compile ImageMagick with Q16 and HDRI support!\n"
          end
          imagemagick_ver =~ /ImageMagick (\d+).(\d+).(\d+)/
          if ($1.to_i < 6) || ($2.to_i < 9 && $1.to_i == 6)
            @magick_available = false
            @versions << "Please use ImageMagick 6.9.3 or higher. Older versions produce bad results.\n"
          end
        end
        @log << @versions
        @versions
      end


      def prepare_dir!
        Dir.mkdir(dir) unless Dir.exist? dir


        mounted_list = `mount`
        if $?.exitstatus != 0
          @using_ramdisk = false
          @log << "'mount' command unavailable. Disk speed may affect benchmark.\n"
        else
          if mounted_list.include? dir
            @using_ramdisk = true
            @log << "Ramdisk already mounted\n"
          else
            @log << "You must execute this benchmark with 'sudo' or a ramdisk cannot be used\n" if ENV['USER'] !=~ /root/
            @log << `mount -t tmpfs -o size=512M tmpfs #{dir}`
            @using_ramdisk = $?.exitstatus == 0
            @log << "Failed to mount ramdisk at #{dir}! Disk speed may affect benchmark.\nTry running `sudo mount -t tmpfs -o size=512M tmpfs #{dir}`\n" unless @using_ramdisk
          end
        end

      end

      def clear_dir!
        exceptions = images.map{|h| Pathname.new(h[:filename]).cleanpath}
        Dir.foreach(dir) do |basename|
          path = Pathname.new(File.join(dir, basename)).cleanpath
          File.delete(path.to_s) unless exceptions.include?(path) || File.directory?(path)
        end
      end

      def fetch_image!(url)
        basename = File.basename(url);

        input_file = "#{dir}/#{basename}"
        unless File.exist? input_file
          @log << "Fetching test image #{url}\n"
          IO.binwrite(input_file, Net::HTTP.get(URI(url)))
        end

        @images << Imageflow::JobContext.get_image_info_by_filename(input_file)
      end

      def fetch_images!
        @image_urls.each do |url|
          fetch_image! url
        end
      end



      def run!
        check_tools!
        prepare_dir!
        fetch_images!
        clear_dir!
        @results = benchmark! log: @log
      end


      def get_cpu_count
        return Java::Java.lang.Runtime.getRuntime.availableProcessors if defined? Java::Java
        return File.read('/proc/cpuinfo').scan(/^processor\s*:/).size if File.exist? '/proc/cpuinfo'
        require 'win32ole'
        WIN32OLE.connect("winmgmts://").ExecQuery("select * from Win32_ComputerSystem").NumberOfProcessors
      rescue LoadError
        Integer `sysctl -n hw.ncpu 2>/dev/null` rescue 1
      end

    end
  end
end
