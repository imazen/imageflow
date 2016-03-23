module Imageflow
  module Riapi
    class ImageLayoutEngine


      #@copy_rect
      #@area_size
      #@target_size

      def initialize(original_size:, crop_rectangle: nil)
        @original_size = original_size
        raise "original_size must be array of two integers" if (original_size.length != 2)
        @original_copy_rect = crop_rectangle
      end



      def apply_instructions(i)
        @manual_crop_rect = i.crop_array.nil? ? @original_copy_rect : ImageLayoutEngine.get_manual_crop_window(instructions: i, original_size: @original_size)
        @manual_crop_rect ||= [0, 0, @original_size[0], @original_size[1]]
        manual_crop_size = [@manual_crop_rect[2] - @manual_crop_rect[0], @manual_crop_rect[3] - @manual_crop_rect[1]]

        @copy_rect = @manual_crop_rect
        if i.width.nil? && i.height.nil? && i.obsolete_maxwidth.nil? && i.obsolete_maxheight.nil?
          #no dimensions!
          @target_size = @area_size = manual_crop_size
        else
          #calculate them
          @target_size = @area_size = nil
          calculate_for_dimensions(i, manual_crop_size: manual_crop_size)
        end
        apply_zoom(i.zoom || 1)
        apply_scale_rules(scale_mode: i.scale, manual_crop_rect: @manual_crop_rect)

        @area_size = @area_size.map { |v| BigDecimal.new([v, 1].max, 0).round }
        @target_size = @target_size.map { |v| BigDecimal.new([v, 1].max, 0).round }
        @target_rect = align_rect( rect: [0,0] + @target_size, container: [0,0] + @area_size, align:  i.anchor || :middle_center)
      end


      #mutates @target_size, @area_size, @copy_rect, and @manual_crop_rect
      def calculate_for_dimensions(i, manual_crop_size:)
        fit_mode = ImageLayoutEngine.determine_fit_mode(instructions: i)
        image_ratio = manual_crop_size[0] / manual_crop_size[1]

        w = i.width
        h = i.height
        mw = i.obsolete_maxwidth
        mh = i.obsolete_maxheight


        #Eliminate cases where both a value and a max value are specified: use the smaller value for the width/height
        if !w.nil? && !mw.nil?
          w = [w, mw].min
          mw = nil
        end
        if !h.nil? && !mh.nil?
          h = [h, mh].min
          mh = nil
        end

        #Handle cases of width/maxheight and height/maxwidth as in legacy version
        mh = [mh, w / image_ratio].min if !w.nil? && !mh.nil?
        mw = [mw, h * image_ratio].min if !h.nil? && !mw.nil?

        w = [w, mw].compact.max
        h = [h, mh].compact.max

        # Calculate missing value (a missing value is handled the same everywhere)
        h = w / image_ratio if h.nil?
        w = h * image_ratio if w.nil?

        if fit_mode == :max
          @area_size = @target_size = scale_inside(inner2: manual_crop_size, outer2: [w, h])
        elsif fit_mode == :pad
          @area_size = [w, h]
          @target_size =scale_inside(inner2: manual_crop_size, outer2: @area_size)
        elsif fit_mode == :crop
          @area_size = @target_size = [w, h]
          cropw_smaller = manual_crop_size[0] <= w
          croph_smaller = manual_crop_size[1] <= h

          new_copy_rect = nil

          if (i.scale == :downscale_only && cropw_smaller != croph_smaller) ||
              (i.scale == :upscale_canvas && (cropw_smaller || croph_smaller))

            @target_size = [[w, manual_crop_size[0]].min, [h, manual_crop_size[1]].min]
            new_copy_rect = @manual_crop_rect = [0, 0, @target_size[0], @target_size[1]]
            @area_size = @target_size if i.scale == :downscale_only
          else
            new_copy_rect = [0, 0] + scale_inside(inner2: @area_size, outer2: manual_crop_size)
          end

          @copy_rect = round_rect(align_rect(rect: new_copy_rect, container: @copy_rect, align: i.anchor || :middle_center))
        else
          #Stretch and carve both act like stretching, so do that:
          @area_size = @target_size = [w, h]
        end

      end

      def round_rect(arr)
        arr.map { |v| BigDecimal.new(v, 0).truncate(0) }
      end

      def apply_zoom(zoom)
        @area_size = @area_size.map { |v| v * zoom }
        @target_size = @target_size.map { |v| v * zoom }
      end

      def elements_are(method: :<=, inner:, outer:)
        inner.zip(outer).all? { |i, o| i.send(method, o) }
      end

      def align_rect(rect:, container:, align:)
        align = align.to_s.split(/_/)
        valign = align[0].to_sym
        halign = align[1].to_sym

        delta_x = 0
        delta_y = 0

        if halign == :left
          delta_x = container[0] - rect[0]
        elsif halign == :center
          delta_x = (container[0] + container[2]) / 2 - (rect[0] + rect[2]) / 2
        elsif halign == :right
          delta_x = container[2] - rect[2]
        end

        if valign == :top
          delta_y = container[1] - rect[1]
        elsif valign == :middle
          delta_y = (container[1] + container[3]) / 2 - (rect[1] + rect[3]) / 2
        elsif valign == :right
          delta_y = container[3] - rect[3]
        end

        [rect[0] + delta_x, rect[1] + delta_y, rect[2] + delta_x, rect[3] + delta_y]
      end

      def scale_inside(inner2:, outer2:)
        outer2 = outer2.map { |v| BigDecimal.new(v, 0) }
        inner2 = inner2.map { |v| BigDecimal.new(v, 0) }
        inner_ratio = inner2[0] / inner2[1]
        outer_ratio = outer2[0] / outer2[1]

        if outer_ratio > inner_ratio
          [inner_ratio * outer2[1], outer2[1]]
        else
          [outer2[0], outer2[0] / inner_ratio]
        end
      end

      def apply_scale_rules(scale_mode:, manual_crop_rect:)
        manual_crop_size = [manual_crop_rect[2] - manual_crop_rect[0], manual_crop_rect[3] - manual_crop_rect[1]]

        if scale_mode == :downscale_only
          if elements_are(method: :<=, inner: manual_crop_size, outer: @target_size)
            @area_size = @target_size = manual_crop_size
            @copy_rect = manual_crop_rect
          end
        elsif scale_mode == :upscale_only
          if elements_are(method: :>, inner: manual_crop_size, outer: @target_size)
            @area_size = @target_size = manual_crop_size
            @copy_rect = manual_crop_rect
          end
        elsif scale_mode == :upscale_canvas #we don't touch area_size here
          if elements_are(method: :<=, inner: manual_crop_size, outer: @target_size)
            @target_size = manual_crop_size
            @copy_rect = manual_crop_rect
          end
        end
      end


      private :apply_zoom, :apply_scale_rules, :calculate_for_dimensions


      def result
        {
            canvas_size: @area_size,
            copy_to_size: @target_size,
            copy_to_rect: @target_rect,
            copy_from: @copy_rect
        }
      end


      def self.determine_fit_mode(instructions:)
        i = instructions
        return i.mode unless i.mode == :none || i.mode.nil?
        return :max if i.width.nil? && i.height.nil?
        return :fill if !i["stretch"].nil? && "fill".casecmp(i["stretch"]) == 0
        return :crop if !i["crop"].nil? && "auto".casecmp(i["crop"]) == 0
        #skipping carve= something other than false/non
        return :pad
      end


      def self.get_manual_crop_window(instructions:, original_size:)
        crop = [0, 0, original_size[0], original_size[1]]
        return crop if instructions.crop_array.nil?

        xunits = instructions.crop_x_units || 0
        yunits = instructions.crop_y_units || 0
        xunits = original_size[0] if xunits == 0
        yunits = original_size[1] if yunits == 0

        crop_array = instructions.crop_array.each_with_index.map do |v, index|

          #Make relative to crop units
          relative_to = BigDecimal.new(index % 2 == 0 ? xunits : yunits, 0)
          max_dimension = BigDecimal.new(index % 2 == 0 ? original_size[0] : original_size[1], 0)
          v = v * max_dimension / relative_to

          #Allow negative offsets
          v += max_dimension if (index < 2 && v < 0) || (index > 1 && v <= 0)

          #Clamp to bounds
          [0, [v, max_dimension].min].max
        end
        #We are not supporting 2-value crop array

        #Fall back to default values if width/height are negative or zero
        return crop if crop_array[3] <= crop_array[1] || crop_array[2] <= crop_array[0]

        crop_array
      end
    end
  end
end


