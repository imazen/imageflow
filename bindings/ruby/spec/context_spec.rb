require 'rspec'
require 'imageflow'
module Imageflow

  describe 'imageflow' do

    describe 'Native' do
      let(:flow) { Imageflow::Native }

      before(:each) do
        @c = flow.context_create
      end

      after(:each) do
        flow.context_destroy(@c)
        @c = nil
      end

      it 'can create and destroy contexts' do
        context = flow.context_create
        expect(context).to_not be_nil
        expect(context.null?).to be_falsey

        flow.context_destroy(context)
      end


      it 'can report an error condition' do
        success = flow.json_response_read(@c, FFI::Pointer.new(0),FFI::Pointer.new(0),FFI::Pointer.new(0),FFI::Pointer.new(0))

        expect(flow.context_has_error(@c)).to be(true)

        expect(success).to be(false)


        buffer = FFI::MemoryPointer.new(:char, 2048, true)

        flow.context_error_and_stacktrace(@c, buffer, 2048, true)

        expect(buffer.read_string).to match /Null argument/
      end
    end

    describe 'Context' do
      before(:each) do
        @c = Context.new
      end

      after(:each) do
        @c.destroy!
      end

      it 'can raise an error' do
        expect {
          @c.call_method(:json_response_read, FFI::Pointer.new(0),FFI::Pointer.new(0),FFI::Pointer.new(0),FFI::Pointer.new(0))
        }.to raise_error /Null argument/
      end
    end
  end
end