module Imageflow
  module Riapi
    class ImageLayoutEngine




      def initialize(original_size:, crop_rectangle: nil)
        @original_size = original_size
        raise "original_size must be array of two integers" if (original_size.length != 2)
        @copy_rect = crop_rectangle
      end


      def apply_instructions(i)
        @copy_rect = ImageLayoutEngine.get_manual_crop_window(instructions: i, original_size: @original_size) unless i.crop_array.nil?

        fit_mode = ImageLayoutEngine.determine_fit_mode(instructions: i)
      end


      def result

      end


      def self.determine_fit_mode(instructions:)
        i = instructions
        return i.mode unless i.mode == :none || i.mode.nil?
        return :max if i.width.nil? && i.height.nil?
        return :fill if "fill".casecmp(i["stretch"]) == 0
        return :crop if "auto".casecmp(i["crop"]) == 0
        #skipping carve= something other than false/non
        return :pad
      end


      def self.get_manual_crop_window(instructions:, original_size: )
        crop = [0,0,original_size[0], original_size[1]]
        return crop if instructions.crop_array.nil?

        xunits = instructions.crop_x_units || 0
        yunits = instructions.crop_y_units || 0
        xunits = original_size[0] if xunits == 0
        yunits = original_size[1] if yunits == 0

        crop_array = instructions.crop_array.each_with_index.map do |v,index|

          #Make relative to crop units
          relative_to = BigDecimal.new(index % 2 == 0 ? xunits : yunits,0)
          max_dimension = BigDecimal.new(index % 2 == 0 ? original_size[0] : original_size[1],0)
          v = v * max_dimension / relative_to

          #Allow negative offsets
          v += max_dimension if (index < 2 && v < 0) || (index > 1 && v <= 0)

          #Clamp to bounds
          [0,[v, max_dimension].min].max
        end
        #We are not supporting 2-value crop array

        #Fall back to default values if width/height are negative or zero
        return crop if crop_array[3] <= crop_array[1] || crop_array[2] <= crop_array[0]

        crop_array
      end
    end
  end
end


