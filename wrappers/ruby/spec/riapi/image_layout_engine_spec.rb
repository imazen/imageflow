require 'rspec'
require 'imageflow'
require 'imageflow/riapi'
module Imageflow::Riapi

  describe 'ImageLayoutEngine' do
    it 'should support mode=crop;scale=canvas' do
      ile = ImageLayoutEngine.new original_size: [1600, 1200], crop_rectangle: nil
      ile.apply_instructions Instructions.new "w=90;h=45;mode=crop;scale=canvas"
      expect(ile.result).to eq({
                                   canvas_size: [90, 45],
                                   copy_to_size: [90, 45],
                                   copy_to_rect: [0, 0, 90, 45],
                                   copy_from: [0, 200, 1600, 1000]
                               })
    end
    it 'should support mode=crop' do
      ile = ImageLayoutEngine.new original_size: [1600, 1200], crop_rectangle: nil
      ile.apply_instructions Instructions.new "w=10;h=10;mode=crop"
      expect(ile.result).to eq({
                                   canvas_size: [10, 10],
                                   copy_to_size: [10, 10],
                                   copy_to_rect: [0, 0, 10, 10],
                                   copy_from: [200, 0, 1400, 1200]
                               })
    end

    it 'should support mode=max' do
      ile = ImageLayoutEngine.new original_size: [1600, 1200], crop_rectangle: nil
      ile.apply_instructions Instructions.new "w=10;h=10;mode=max"
      expect(ile.result).to eq({
                                   canvas_size: [10, 8],
                                   copy_to_size: [10, 8],
                                   copy_to_rect: [0, 0, 10, 8],
                                   copy_from: [0, 0, 1600, 1200]
                               })
    end

    it 'should support mode=max implicitly' do
      ile = ImageLayoutEngine.new original_size: [1600, 1200], crop_rectangle: nil
      ile.apply_instructions Instructions.new "w=10"
      expect(ile.result).to eq({
                                   canvas_size: [10, 8],
                                   copy_to_size: [10, 8],
                                   copy_to_rect: [0, 0, 10, 8],
                                   copy_from: [0, 0, 1600, 1200]
                               })
    end

    describe "#get_manual_crop_window" do
      it "should work with percentages" do
        crop = ImageLayoutEngine.get_manual_crop_window original_size: [100, 100],
                                                        instructions: Instructions.new("?cropxunits=1&cropyunits=1&crop=0.2,0.2,0.8,0.8")
        expect(crop).to eq([20, 20, 80, 80])

      end
      #TODO: add more tests here for crop window calculations
    end


    describe "#scale_inside" do

      it "should not produce a size larger than the container" do
        result = ImageLayoutEngine.new(original_size: [1600, 1200]).scale_inside inner2: [10, 10], outer2: [1600, 1200]
        expect(result).to eq([1200, 1200])
      end
    end
  end
end
