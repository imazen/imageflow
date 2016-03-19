require 'rspec'
require 'imageflow'
module Imageflow

  describe 'imageflow' do

    describe 'Native' do
      let(:flow) {Imageflow::Native}

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
        bitmap = flow.bitmap_bgra_create_header(@c, -1, -1) #invalid size

        expect(flow.context_has_error(@c)).to be(true)

        expect(bitmap.null?).to be(true)


        buffer = FFI::MemoryPointer.new(:char, 2048, true)

        flow.context_error_message(@c, buffer, 2048)

        expect(buffer.read_string).to match /status_code: 2/
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
          @c.call_method(:bitmap_bgra_create_header, -1, -1)
        }.to raise_error /status_code: 2/
      end

      describe 'Graph' do
        it 'can be created' do
          g = @c.create_graph
        end
        it 'can be destroyed' do
          g = @c.create_graph
          g.destroy!
          expect(g.destroyed?).to be true
        end

        it 'can be populated' do
          g = @c.create_graph

          g.create_node(:canvas, :bgra32, 400, 300, 0xFFFFFFFF)
          .add(:scale, 300, 200)
          .add(:resource_placeholder, 0)

        end
      end
    end
  end
end