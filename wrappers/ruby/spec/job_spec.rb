require 'rspec'
require 'imageflow'
module Imageflow

  describe 'imageflow' do
    describe 'Job' do
      before(:each) do
        @c = Context.new
      end

      after(:each) do
        @c.destroy!
      end


      it 'can be created' do
        @c.create_job
      end
      it 'can be destroyed' do
        j = @c.create_job
        j.destroy!
        expect(j.destroyed?).to be true
      end

      it 'can be populated' do
        job = @c.create_job

        bytes = "\x89\x50\x4E\x47\x0D\x0A\x1A\x0A\x00\x00\x00\x0D\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1F\x15\xC4\x89\x00\x00\x00\x0A\x49\x44\x41\x54\x78\x9C\x63\x00\x01\x00\x00\x05\x00\x01\x0D\x0A\x2D\xB4\x00\x00\x00\x00\x49\x45\x4E\x44\xAE\x42\x60\x82".b
        job.add_input_buffer(placeholder_id: 0, bytes: bytes)
        output_resource_id = job.add_output_buffer(placeholder_id: 1)

        g = @c.create_graph

        g.create_node(:resource_placeholder, 0)
            .add(:scale, 300, 200)
            .add(:encoder_placeholder, 1, :encode_jpeg)


        job.execute graph: g.deep_clone

        out_bytes = job.get_buffer(resource_id: output_resource_id)


      end
    end
  end
end
