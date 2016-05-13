require 'rspec'
require 'imageflow'
require 'imageflow/vips'
require 'imageflow/vips/benchmark'
module Imageflow::Mimick

  describe 'imageflow-vips' do

    it 'should have very similar quality' do



      c = [{linear: true, w: 400, format: :png}, {linear: true, w: 2560, format: :jpg}, {linear: false, w: 2560, format: :jpg}]
      urls = ["u1.jpg", "u6.jpg"].map do |name|
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/#{name}"
      end\

      #benchmarkees = [Imageflow::Vips::ImageflowVips,Imageflow::Vips::VipsThumbnail, Imageflow::Vips::ImageflowVipsNoBlockscale, Imageflow::Vips::Reference]
      benchmarkees = [Imageflow::Vips::ImageflowVips, Imageflow::Vips::ImageflowVipsNoBlockscale, Imageflow::Vips::Reference]



      Imageflow::Vips::Benchmark.new(output_stream: STDOUT, configs: c, image_urls: urls, benchmarkees: benchmarkees ).run!

    end
  end
end






#compare /home/n/Documents/imazen/imageflow/wrappers/ruby/temp_vips_benchmarking/reference_0_u1_400__23912315f.png /home/n/Documents/imazen/imageflow/wrappers/ruby/temp_vips_benchmarking/flow_0_u1_400__23912315f.png -fuzz 2%  x: