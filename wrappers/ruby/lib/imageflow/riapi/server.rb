require 'rubygems'
require 'ffi'
require 'sinatra'

module Imageflow
  module Riapi
    class Server < Sinatra::Base


      get '/hi' do

        c = Context.new
        job = c.create_job



        bytes =Net::HTTP.get(URI("http://z.zr.io/ri/8s.jpg?width=800"))


        job.add_input_buffer(placeholder_id: 0, bytes: bytes)
        output_resource_id = job.add_output_buffer(placeholder_id: 1)

        g = c.create_graph

        g.create_node(:resource_placeholder, 0)
            .add(:scale, (params[:width] || 300).to_i, (params[:height] || 200).to_i)
            .add(:encoder_placeholder, 1, :encode_jpeg)


        job.execute graph: g.deep_clone

        out_bytes = job.get_buffer_bytes(resource_id: output_resource_id)

        c.destroy!

        content_type 'image/jpeg'
        out_bytes
      end
    end
  end
end
