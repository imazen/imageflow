module Imageflow
  module Riapi
    class GraphBuilder
      def initialize(context:)
        @context = context
        @steps = []
      end

      attr_accessor :result_mime_type

      def add_decoder(io_id:)
        @steps << {decode: {ioId: io_id}}
      end

      def add_encoder(io_id:, codec: , instructions: )
        preset = {"LibjpegTurbo": {}} if codec == :jpg
        preset = {"Libpng": {}} if codec == :png

        @steps << {encode: {ioId: io_id, preset: preset}}
      end

      def add_scale(w:,h:, down_filter: :filter_Robidoux, up_filter: :filter_Robidoux)
        @steps <<  {"resample2d": {
            "w": w,
            "h": h,
            "downFilter": "Robidoux",
            "upFilter": "Robidoux",
            "hints": nil
        }}
      end

      def add_rotate(degress)
        raise "Invalid degree value (#{degrees}) - must be multiple of 90" unless degress % 90 == 0
        degrees = degrees % 360
        return if degress == 0
        @steps << {"rotate#{degress}": {}}
      end

      def add_flip(flip)
        accepted = [:none, :x, :y, :xy]
        raise "Invalid flip value (#{degrees}) - must be one of #{accepted.inspect}" unless accepted.include? (flip)
        return if flip == :none
        @steps << {flipH: {}} if flip.to_s.start_with?("x")
        @steps << {flipV: {}} if flip.to_s.end_width?("y")
      end

      def add_crop(crop:)
        @steps << {"crop": {
            "x1": crop[0],
            "y1": crop[1],
            "x2": crop[2],
            "y2": crop[3]
        }}
      end

      def add_expand_canvas(left:, top:, right:, bottom:, color:)
        @steps <<  {"expandCanvas": {
            "left": left,
            "top": top,
            "right": right,
            "bottom": bottom,
            "color": {
                "Srgb": {
                    "Hex": color
                }
            }
        }}
      end

      def framewise
        {steps: @steps}
      end

      def apply_decoder_scaling_and_get_dimensions(source_info:, instructions:, job:, input_placeholder_id:)
        original_size = [source_info[:frame0_width], source_info[:frame0_height]]

        #swap coords if we're rotating
        original_size.reverse! unless instructions.source_rotate.nil? || instructions.source_rotate % 180 == 0

        ile = ImageLayoutEngine.new original_size: original_size
        ile.apply_instructions instructions

        min_precise_scaling = instructions.precise_scaling_ratio || 3.0
        trigger_ratio = min_precise_scaling
        crop_ratios = [original_size[0].to_f / (ile.result[:copy_from][2] - ile.result[:copy_from][0]).to_f, original_size[1].to_f / (ile.result[:copy_from][3] - ile.result[:copy_from][1]).to_f]
        target_decoder_size = ile.result[:copy_to_size].zip(crop_ratios).map {|v, ratio| (v.to_f * ratio.to_f * min_precise_scaling)}
        #swap coords if we're rotating
        target_decoder_size.reverse! unless instructions.source_rotate.nil? || instructions.source_rotate % 180 == 0
        trigger_decoder_scaling = target_decoder_size.map{|v| v.to_f * trigger_ratio / min_precise_scaling}

        gamma_correct_for_srgb_during_spatial_luma_scaling = instructions.jpeg_idct_downscale_linear.nil? ? (instructions.floatspace == :linear) : instructions.jpeg_idct_downscale_linear

        job.set_decoder_downscale_hints(placeholder_id: input_placeholder_id,
                                        downscaled_min_width: target_decoder_size[0].to_i,
                                        downscaled_min_height: target_decoder_size[1].to_i,
                                        scale_luma_spatially: gamma_correct_for_srgb_during_spatial_luma_scaling,
                                        gamma_correct_for_srgb_during_spatial_luma_scaling: gamma_correct_for_srgb_during_spatial_luma_scaling)

        updated_info = job.get_image_info(placeholder_id: input_placeholder_id)

        [updated_info[:frame0_width],updated_info[:frame0_height]]
      end


      def build_framewise(job:, input_placeholder_id:, output_placeholder_id:, instructions:, source_info:)

        original_size = apply_decoder_scaling_and_get_dimensions source_info: source_info, instructions: instructions, job: job, input_placeholder_id: input_placeholder_id

        #TODO: apply autorotate & autorotate.default (false)
        i = instructions

        add_decoder(io_id: 0)

        #swap coords if we're rotating
        original_size.reverse! unless i.source_rotate.nil? || i.source_rotate % 180 == 0

        ile = ImageLayoutEngine.new original_size: original_size
        ile.apply_instructions instructions

        add_rotate(i.source_rotate) unless i.source_rotate.nil?
        add_flip(i.source_flip) unless i.source_flip.nil?


        add_crop(crop: ile.result[:copy_from])

        add_scale(w: ile.result[:copy_to_size][0], h: ile.result[:copy_to_size][1])

        add_expand_canvas(left: ile.result[:copy_to_rect][0],
                          top: ile.result[:copy_to_rect][1],
                          right: ile.result[:canvas_size][0] - ile.result[:copy_to_size][0] - ile.result[:copy_to_rect][0],
                          bottom: ile.result[:canvas_size][1] - ile.result[:copy_to_size][1] - ile.result[:copy_to_rect][1],
                          color: "FFFFFFFF") #instructions.background_color)


        #TODO: Add parsing for these
        # if (!s.settings.Padding.IsEmpty) {
        #     s.layout.AddRing("padding",  s.settings.Padding);
        # }
        # //And borders
        # if (!s.settings.Border.IsEmpty) {
        #     s.layout.AddRing("border", s.settings.Border);
        # }
        #
        # if (!s.settings.Margin.IsEmpty) {
        #     s.layout.AddRing("margin", s.settings.Margin);
        # }


        add_rotate(i.rotate) unless i.rotate.nil?
        add_flip(i.flip) unless i.flip.nil?


        if instructions.format == :png || instructions.format.nil? && source_info[:preferred_mime_type] == "image/png"
          @result_mime_type = 'image/png'
          output_codec = :png #:encode_png
        else
          @result_mime_type = 'image/jpeg'
          output_codec = :jpg #:encode_jpeg
        end

        add_encoder(io_id: output_placeholder_id, codec: output_codec, instructions: instructions)

        self.framewise
      end
    end
  end
end

