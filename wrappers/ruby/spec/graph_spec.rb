require 'rspec'
require 'imageflow'
module Imageflow

  describe 'imageflow' do
    describe 'Graph' do
      before(:each) do
        @c = Context.new
      end

      after(:each) do
        @c.destroy!
      end


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
            .add(:scale, 300, 200, :filter_Robidoux, :filter_Robidoux)
            .add(:encoder_placeholder, 0)

      end
    end
  end
end
