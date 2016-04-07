require 'rspec'
require 'imageflow'
require 'imageflow/vips'

module Imageflow::Vips

  describe 'imageflow-vips' do
    it 'should be of similar speed' do

      dir = File.expand_path("temp_vips_benchmarking")
      Dir.mkdir(dir) unless Dir.exist? dir

      input_file = "#{dir}/u1.jpg"
      url = "http://s3.amazonaws.com/resizer-images/u1.jpg"
      IO.binwrite(input_file,Net::HTTP.get(URI(url))) unless File.exist? input_file


      executables = ["vipsthumbnail", File.expand_path("../../bin/imageflow-vips", __FILE__)]

      output_format = "#{dir}/thumb_%s.jpg"

      argument_sets = ["", "--linear"]

      argument_sets.each do |argset|
        puts "\nComparing both with arguments: #{argset}\n"
        Benchmark.bmbm(80) do |bench|
          executables.each do |program|

            args = "#{argset} --output=#{output_format} #{input_file}"
            puts "Using #{program} #{args}\n"

            bench.report(File.basename(program)) do
              result = 0
              out_msg = "";

              #exclude loading rubygems
              if program =~ /imageflow-vips/
                result = Imageflow::Vips::Cli.start(args.split(/\s+/))
              else
                out_msg = `#{program} #{args}`
                result = $?.exitstatus
              end
              if result != 0
                raise "#{out_msg}\nFailed with exit code #{result}: #{program} #{args}"
              end
            end
          end
        end
      end



    end
  end
end
