require 'rspec'
require 'imageflow'
require 'imageflow/riapi'
module Imageflow
  module Riapi

    describe 'ClassicJob' do
      it 'should work' do
        bytes = "\x89\x50\x4E\x47\x0D\x0A\x1A\x0A\x00\x00\x00\x0D\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1F\x15\xC4\x89\x00\x00\x00\x0A\x49\x44\x41\x54\x78\x9C\x63\x00\x01\x00\x00\x05\x00\x01\x0D\x0A\x2D\xB4\x00\x00\x00\x00\x49\x45\x4E\x44\xAE\x42\x60\x82".b

        source = ImageSource.from_binary(binary_string: bytes)
        job = ClassicJob.new image_source: source, instructions: Instructions.new("w=90;h=45;mode=crop;scale=canvas")
        job.acquire_bytes!

        job.result_bytes
      end
    end
  end
end
