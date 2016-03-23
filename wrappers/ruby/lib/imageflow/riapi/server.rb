require 'rubygems'
require 'ffi'
require 'sinatra'
require 'benchmark'

module Imageflow
  module Riapi
    class Server < Sinatra::Base

      def process_bytes_and_hash(bytes:, hash:)
        job = nil
        cpu_time = Benchmark.measure do
          job = ClassicJob.new image_source: ImageSource.from_binary(binary_string: bytes),
                               instructions: Instructions.new(hash)

          job.acquire_bytes!
        end

        response.headers['X-CPU-TIME'] = "%.2f" % (cpu_time.total.to_f * 1000.0)
        content_type job.result_content_type

        job.result_bytes
      end


      get '/tinypng' do

        bytes = "\x89\x50\x4E\x47\x0D\x0A\x1A\x0A\x00\x00\x00\x0D\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1F\x15\xC4\x89\x00\x00\x00\x0A\x49\x44\x41\x54\x78\x9C\x63\x00\x01\x00\x00\x05\x00\x01\x0D\x0A\x2D\xB4\x00\x00\x00\x00\x49\x45\x4E\x44\xAE\x42\x60\x82".b

        process_bytes_and_hash(bytes: bytes, hash: request.env['rack.request.query_hash'])

      end


      get '/ri/:path' do |path|
        @@cache ||= {}
        #url = "http://z.zr.io/ri/#{path}?width=1600&quality=80"
        url = "http://s3.amazonaws.com/resizer-images/#{path}"
        @@cache[url] ||= Net::HTTP.get(URI(url))

        process_bytes_and_hash(bytes: @@cache[url], hash: request.env['rack.request.query_hash'])
      end
    end
  end
end
