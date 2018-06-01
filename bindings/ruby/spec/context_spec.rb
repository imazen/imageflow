require 'rspec'
require 'imageflow'
module Imageflow

  describe 'imageflow' do

    describe 'Native' do
      let(:flow) { Imageflow::Native }

      before(:each) do
        @c = flow.context_create(3,0)
      end

      after(:each) do
        flow.context_destroy(@c)
        @c = nil
      end

      it 'can create and destroy contexts' do
        context = flow.context_create(3,0)
        expect(context).to_not be_nil
        expect(context.null?).to be_falsey

        flow.context_destroy(context)
      end


      it 'can report an error condition' do
        success = flow.json_response_read(@c, FFI::Pointer.new(0),FFI::Pointer.new(0),FFI::Pointer.new(0),FFI::Pointer.new(0))

        expect(flow.context_has_error(@c)).to be(true)

        expect(success).to be(false)


        buffer = FFI::MemoryPointer.new(:char, 2048, true)

        flow.context_error_write_to_buffer(@c, buffer, 2048, nil)

        expect(buffer.read_string).to match /NullArgument/
        expect(buffer.read_string).to match /imageflow_abi/
      end
    end

    describe 'Context' do
      def c
        @c
      end
      before(:each) do
        @c = JobContext.new
      end

      after(:each) do
        @c.destroy!
      end

      it 'can raise an error' do
        expect {
          c.get_buffer(io_id: 27)
        }.to raise_error /ArgumentInvalid/
      end

      it 'can be populated' do

        bytes = "\x89\x50\x4E\x47\x0D\x0A\x1A\x0A\x00\x00\x00\x0D\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1F\x15\xC4\x89\x00\x00\x00\x0A\x49\x44\x41\x54\x78\x9C\x63\x00\x01\x00\x00\x05\x00\x01\x0D\x0A\x2D\xB4\x00\x00\x00\x00\x49\x45\x4E\x44\xAE\x42\x60\x82".b
        c.add_input_buffer(io_id: 0, bytes: bytes)
        c.add_output_buffer(io_id: 1)


        c.execute framewise: {steps: [
            {decode: {io_id: 0}},
            {resample_2d: {w: 300, h: 200}},
            {encode: {io_id: 1, preset: {"libjpeg": {quality: 90}}}}
        ]}

        out_bytes = c.get_buffer(io_id: 1)

        expect(out_bytes[:buffer_size]).to be_between(200, 900)

      end

      it 'can write to a file, then read from it' do

        bytes = "\x89\x50\x4E\x47\x0D\x0A\x1A\x0A\x00\x00\x00\x0D\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1F\x15\xC4\x89\x00\x00\x00\x0A\x49\x44\x41\x54\x78\x9C\x63\x00\x01\x00\x00\x05\x00\x01\x0D\x0A\x2D\xB4\x00\x00\x00\x00\x49\x45\x4E\x44\xAE\x42\x60\x82".b
        c.add_input_buffer(io_id: 0, bytes: bytes)
        #c.add_output_file(io_id: 1, filename: "hello.png")
        c.add_output_buffer(io_id: 1)

        c.execute framewise: {steps: [
            {decode: {io_id: 0}},
            {resample_2d: {w: 300, h: 200}},
            {encode: {io_id: 1, preset: {"libpng": {}}}}
        ]}

        c.write_output_buffer_to_file(io_id: 1, filename: "hello.png")
        #reset the context to ensure the file stream is closed
        @c.destroy!
        @c = Imageflow::JobContext.new

        c.add_input_buffer_from_file(io_id:0, filename: "hello.png")
        #c.add_input_file(io_id: 0, filename: "hello.png")
        c.add_output_buffer(io_id: 1)
        c.execute framewise: {steps: [
            {decode: {io_id: 0}},
            {resample_2d: {w: 300, h: 200}},
            {encode: {io_id: 1, preset: {"libjpeg": {}}}}
        ]}

        out_bytes = c.get_buffer(io_id: 1)

        expect(out_bytes[:buffer_size]).to be_between(200, 900)

        File.delete("hello.png")
      end
    end
  end
end