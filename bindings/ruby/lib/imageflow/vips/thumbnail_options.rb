module Imageflow
  module Vips
    class ThumbnailOptions
      def initialize
        @delete_profile = false
        @linear = false
        @crop_image = false
        @rotate_image = false
        @thumbnail_size_str = "128"
        @width = 128
        @height = 128
        @output_format_string = "tn_%s.jpg"
        @import_profile = @export_profile = nil
        @stop_block_scaling_at = nil
        @input_files = []
      end

      attr_accessor :thumbnail_size_str, :width, :height, :output_format_string, :import_profile, :export_profile, :delete_profile, :linear, :crop_image, :rotate_image, :input_files, :stop_block_scaling_at
    end
  end
end