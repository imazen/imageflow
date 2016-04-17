require 'rspec'
require 'imageflow'
require 'imageflow/vips'
require 'imageflow/vips/benchmark'
module Imageflow::Vips

  describe 'imageflow-vips' do

    it 'should run benchmarks' do

      #Setting blocking:false made total CPU time plummet, but real time triple. Much overhead may be in ruby thread switching
      c = [{linear: true, w: 400, format: :jpg}]
      urls = ["u1.jpg", "u6.jpg","u1.jpg", "u6.jpg","u1.jpg", "u6.jpg","u1.jpg", "u6.jpg","u1.jpg", "u6.jpg","u1.jpg", "u6.jpg","u1.jpg", "u6.jpg","u1.jpg", "u6.jpg"].map do |name|
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/#{name}"
      end

      Imageflow::Vips::Benchmark.new(output_stream: STDOUT, configs: c, image_urls: urls).run!

    end
  end
end
