module Imageflow
  module Riapi
    class GraphBuilder
      def initialize(context:)
        @context = context
      end

      attr_accessor :result_mime_type

      def add_rotate(degress)
        raise "Invalid degree value (#{degrees}) - must be multiple of 90" unless degress % 90 == 0
        degrees = degrees % 360
        return if degress == 0
        @last = @last.add("rotate_#{degrees}".to_sym)
      end

      def add_flip(flip)
        accepted = [:none, :x, :y, :xy]
        raise "Invalid flip value (#{degrees}) - must be one of #{accepted.inspect}" unless accepted.include? (flip)
        return if flip == :none
        @last = @last.add(:primitive_flip_horizontal) if flip.to_s.start_with?("x")
        @last = @last.add(:primitive_flip_vertical) if flip.to_s.end_width?("y")
      end

      def add_crop(crop:)
        @last = @last.add(:primitive_crop, crop[0], crop[1], crop[2], crop[3] )
      end

      def add_expand_canvas(left:, top:, right:, bottom:, color: )
        @last = @last.add(:expand_canvas, left,top,right, bottom, color )
      end


      def build_graph(input_placeholder_id: , output_placeholder_id: , instructions:, source_info:)

        #TODO: apply autorotate & autorotate.default (false)
        g = @context.create_graph
        i = instructions

        @last = g.create_node(:resource_placeholder, 0)


        original_size = [source_info[:frame0_width], source_info[:frame0_height]]

        #swap coords if we're rotating
        original_size.reverse unless i.source_rotate.nil? || i.source_rotate % 180 == 0


        add_rotate(i.source_rotate) unless i.source_rotate.nil?
        add_flip(i.source_flip) unless i.source_flip.nil?


        ile = ImageLayoutEngine.new original_size: original_size
        ile.apply_instructions instructions

        add_crop(crop: ile.result[:copy_from])


        @last = @last.add(:scale, ile.result[:copy_to_size][0], ile.result[:copy_to_size][1])

        add_expand_canvas(left: ile.result[:copy_to_rect][0],
                          top: ile.result[:copy_to_rect][1],
                          right: ile.result[:canvas_size][0] - ile.result[:copy_to_size][0],
                          bottom: ile.result[:canvas_size][1] - ile.result[:copy_to_size][1],
                          color: 0xFFFFFFFF) #instructions.background_color)



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








        if source_info[:codec_type] == :decode_png
          @result_mime_type = 'image/png'
          output_codec = :encode_png
        else
          @result_mime_type = 'image/jpeg'
          output_codec = :encode_jpeg
        end
        @last = @last.add(:encoder_placeholder, output_placeholder_id, output_codec)

        g
      end
    end
  end
end

