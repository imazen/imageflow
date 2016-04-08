require 'rspec'
require 'imageflow'
require 'imageflow/vips'

module Imageflow::Vips

  describe 'imageflow-vips' do
    it 'should be of similar speed' do

      dir = File.expand_path("temp_vips_benchmarking")
      Dir.mkdir(dir) unless Dir.exist? dir

      input_file_basename = "u1.jpg"

      input_file = "#{dir}/#{input_file_basename}"
      url = "http://s3.amazonaws.com/resizer-images/#{input_file_basename}"
      puts "Fetching test image #{url}\n"
      IO.binwrite(input_file, Net::HTTP.get(URI(url))) unless File.exist? input_file


      puts `vipsthumbnail --vips-version`
      skip if $?.exitstatus != 0

      executables = ["vipsthumbnail", "imageflow-vips"]

      output_path_for = lambda { |exe, index| exe =~ /imageflow-vips/ ? "#{dir}/imageflow_%s_#{index}.png" : "#{dir}/vips_%s_#{index}.png" }

      argument_sets = ["", "--linear"]

      argument_sets.each_with_index do |argset, index|
        puts "\nComparing both with arguments: #{argset}\n"
        Benchmark.bmbm(80) do |bench|
          executables.each do |program|

            args = "#{argset} --output=#{output_path_for.call(program, index)} #{input_file}"
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

        sleep(1)

        paths = executables.map { |exe| output_path_for.call(exe, index).gsub(/%s/, input_file_basename.gsub(/\.[a-zA-Z]+$/, '')) }
        puts "\nComparing result images...\n"
        puts `dssim #{paths[0]} #{paths[1]}`
        puts "            #{paths[0]}\n"

      end
    end
  end
end
