require 'bigdecimal'
module Imageflow
  module Riapi
    class Instructions
      attr_accessor :hash

      def initialize(input = {})
        if input.is_a? Hash
          @hash = input
        elsif input.is_a? String
          @hash = Url.parse_query_string_implicit(input, allow_semicolons: true)
        else
          raise "Unsupported input for Instructions.new - provide a String or Hash"
        end
        raise "What What?" if @hash.nil?
      end


      def to_s
        Url.build_query_string hash_of_strings: hash, url_encode: false, first_separator: ";", later_separators: ";"
      end

      def to_query
        Url.build_query_string hash_of_strings: hash, url_encode: true
      end


      def normalize(keys:)
        keys = [keys].flatten
        keys_n = keys.map { |k| k.to_s.upcase }
        values = []
        hash.each_pair do |k, v|
          priority = keys_n.index k.to_s.upcase
          unless priority.nil?
            values << [v, priority]
            hash.delete(k)
          end
        end
        values.sort_by! { |k, v| v } #Sort by priority

        hash[keys[0].to_s] = values[0][0] unless values.empty? ##take the first one (current strategy)
      end

      def [](key)
        key = key.to_s.upcase
        hash.each_pair do |k, v|
          return v if k.to_s.upcase == key
        end
        nil
      end

      def []=(k, v)
        set_first(keys: k, value: v)
      end


      def get_first(keys:)
        keys = [keys].flatten
        keys_n = keys.map { |k| k.to_s.upcase }
        values = []
        hash.each_pair do |k, v|
          priority = keys_n.index k.to_s.upcase
          unless priority.nil?
            values << [v, priority]
          end
        end
        values.sort_by! { |k, v| v } #Sort by priority
        values.empty? ? nil : values[0][0] ##take the first one (current strategy)
      end

      def set_first(keys:, value:)
        keys = [keys].flatten
        normalize(keys: keys)
        if value.nil?
          hash.delete keys[0].to_s
        else
          hash[keys[0].to_s] = value
        end
      end

      def self.parse_decimal_strict(v)
        Float(v)
        BigDecimal(v, 0)
      rescue ArgumentError
        nil

      end

      def self.clamp_decimal(v, min: nil, max: nil)
        v = Instructions.parse_decimal_strict(v)
        v = [v, BigDecimal.new(min)].max unless min.nil?
        v = [v, BigDecimal.new(max)].min unless max.nil?
        v
      end

      def self.parse_bool(str)
        return nil if str.nil?
        str = str.to_s.downcase
        return true if ["true", "1", "yes", "on"].include? str
        return false if ["false", "0", "no", "off"].include? str
        return nil
      end


      def self.attr_hash_int(name, keys:, min: nil, max: nil)
        class_eval do
          define_method(name) do
            v = get_first(keys: keys)
            v.nil? ? nil : Instructions.clamp_decimal(v, min: min, max: max).round.to_i
          end
          define_method("#{name}=".to_sym) do |v|
            raise "Must be set to nil or numeric value" unless v.nil? || v.is_a?(Numeric)
            set_first(keys: keys, value: v.nil? ? nil : Instructions.clamp_decimal(v, min: min, max: max).round.to_i.to_s)
          end
        end
      end


      def self.attr_hash_decimal(name, keys:, min: nil, max: nil)
        class_eval do
          define_method(name) do
            v = get_first(keys: keys)
            v.nil? ? nil : Instructions.clamp_decimal(v, min: min, max: max)
          end
          define_method("#{name}=".to_sym) do |v|
            raise "Must be set to nil or numeric value" unless v.nil? || v.is_a?(Numeric)
            set_first(keys: keys, value: v.nil? ? nil : Instructions.clamp_decimal(v, min: min, max: max).round(10).to_s('F'))
          end
        end
      end

      def self.attr_hash_string(name, keys:)
        class_eval do
          define_method(name) do
            get_first(keys: keys)
          end
          define_method("#{name}=".to_sym) do |v|
            set_first(keys: keys, value: v.nil? ? nil : v.to_s)
          end
        end
      end

      def self.attr_hash_bool(name, keys:)
        class_eval do
          define_method(name) do
            Instructions.parse_bool(get_first(keys: keys))
          end
          define_method("#{name}=".to_sym) do |v|
            raise "Must be set to nil or a boolean value" unless v.nil? || v.is_a?(TrueClass) || v.is_a?(FalseClass)
            set_first(keys: keys, value: v.nil? ? nil : v.to_s)
          end
        end
      end

      def self.parse_enum(v, map:)
        return nil if v.nil?
        v = v.to_s.downcase
        map[v]
      end

      def self.parse_list(v, default_element: nil, value_type: :string, permitted_counts: nil)
        return nil if v.nil?
        v.gsub! /^[ \(\),]+/, ""
        v.gsub! /[ \(\),]+$/, "" #TODO: should we actually be stripping commas? Don't think so, but that is current IR behavior
        parts = v.split(/,/)
        #Gotta match the counts
        return nil unless permitted_counts.nil? || permitted_counts.include?(parts.length)

        parts.map do |v|
          v = v.empty? ? nil : v
          return nil if v.nil? && default_element.nil?
          if value_type == :decimal && !v.nil?
            v = Instructions.parse_decimal_strict(v)
          end
          v || default_element
        end
      end

      def self.stringify_enum(v, values:)
        raise "Value must be one of #{values.inspect} or nil" unless v.nil? || values.include?(v)
        v.nil? ? nil : v.to_s.gsub(/_/, "")
      end

      def self.attr_hash_enum(name, keys:, values:, map: {})
        map2 = Hash[map.map { |k, v| [k.to_s.downcase, v] } + values.map { |v| [v.to_s.downcase, v] } + values.map { |v| [v.to_s.gsub(/_/, '').downcase, v] }]
        class_eval do
          define_method(name) do
            Instructions.parse_enum(get_first(keys: keys), map: map2)
          end
          define_method("#{name}=".to_sym) do |v|
            set_first(keys: keys, value: Instructions.stringify_enum(v, values: values))
          end
        end
      end


      attr_hash_int :width, keys: ["width", "w"]
      attr_hash_int :height, keys: ["height", "h"]

      attr_hash_int :obsolete_maxwidth, keys: ["maxwidth"]
      attr_hash_int :obsolete_maxheight, keys: ["maxheight"]

      attr_hash_enum :mode, keys: "mode",
                     values: [:none, :max, :pad, :crop, :stretch] #we're omitting :carve

      attr_hash_enum :anchor, keys: "anchor",
                     values: [:top_left, :top_center, :top_right, :middle_left, :middle_center, :middle_right, :bottom_left, :bottom_center, :bottom_right]

      attr_hash_enum :source_flip, keys: "sflip",
                     values: [:none, :x, :y, :xy],
                     map: {h: :x, v: :y, both: :xy}

      attr_hash_enum :flip, keys: "flip",
                     values: [:none, :x, :y, :xy],
                     map: {h: :x, v: :y, both: :xy}

      attr_hash_enum :scale, keys: "scale",
                     values: [:both, :upscale_canvas, :upscale_only, :downscale_only],
                     map: {canvas: :upscale_canvas, up: :upscale_only, down: :downscale_only}

      attr_hash_enum :cache, keys: "cache",
                     values: [:no, :always, :default]

      attr_hash_enum :process, keys: "process",
                     values: [:no, :always, :default]


      attr_hash_int :frame, keys: "frame"
      attr_hash_int :page, keys: "page"

      attr_hash_int :jpeg_quality, keys: "quality", min: 0, max: 100

      attr_hash_enum :jpeg_subsampling, keys: "subsampling",
                     values: [:default, :y4cb1cr1, :y4cb2cr0, :y4cb2cr2, :y4cb4cr4],
                     map: {"411": :y4cb1cr1, "420": :y4cb2cr0, "422": :y4cb2cr2, "444": :y4cb4cr4}


      attr_hash_int :palette_size, keys: "colors", min: 0, max: 256
      attr_hash_decimal :zoom, keys: "zoom"
      attr_hash_decimal :crop_x_units, keys: "cropxunits"
      attr_hash_decimal :crop_y_units, keys: "cropyunits"

      #todo: croprectangle []

      def crop_array
        Instructions.parse_list(get_first(keys: "crop"), value_type: :decimal, permitted_counts: [4], default_element: 0)
      end

      def crop_array=(v)
        raise "nil or Array of 4 BigDecimal values required for crop_array" unless v.nil? || (v.length == 4 && v.all? { |e| e.is_a?(Numeric) })
        set_first(keys: "crop", value: v.nil? ? nil : v.map { |d| BigDecimal(d, 0).round(10).to_s('F') }.join(','))
      end


      # /// <summary>
      # /// An X1,Y1,X2,Y2 array of coordinates. Unless CropXUnits and CropYUnits are specified, these are in the coordinate space of the original image.
      # /// </summary>
      # public double[] CropRectangle { get { return this.GetList<double>("crop", 0, 4); } set { this.SetList("crop", value, true, 4); } }


      attr_hash_bool :autorotate, keys: "autorotate"


      attr_hash_int :source_rotate, keys: "srotate"
      attr_hash_int :rotate, keys: "rotate"


      attr_hash_string :format_str, keys: ["format", "thumbnail"]

      attr_hash_enum :format, keys: ["format", "thumbnail"],
                     values: [:jpeg, :png, :gif],
                     map: {jpg: :jpeg, jpe: :jpeg, jif: :jpeg, jfif: :jpeg, jfi: :jpeg, exif: :jpeg}


      attr_hash_bool :ignore_icc, keys: "ignoreicc"

      ##skipping FallbackImage
      #attr_hash_string :fallback_image, keys: "404"

      attr_hash_string :background_color, keys: "bgcolor"
      attr_hash_string :padding_color, keys: "paddingcolor"
      attr_hash_string :border_color, keys: "bordercolor"
      attr_hash_string :preset, keys: "preset"

      #skipping watermark
      #attr_hash_string :watermark, keys: "watermark"

      #
      # /// <summary>
      # /// Applies a Negative filter to the image. Requires the SimpleFilters plugin
      # /// </summary>
      # public bool? Invert { get { return this.Get<bool>("s.invert"); } set { this.Set<bool>("s.invert", value); } }
      #
      #
      # /// <summary>
      # /// Applies a Sepia filter to the image. Requires the SimpleFilters plugin
      # /// </summary>
      # public bool? Sepia { get { return this.Get<bool>("s.sepia"); } set { this.Set<bool>("s.sepia", value); } }
      #
      # /// <summary>
      # /// Applies the specified kind of grayscale filter to the image. Requires the SimpleFilters plugin
      # /// </summary>
      # public GrayscaleMode? Grayscale { get { return this.Get<GrayscaleMode>("s.grayscale"); } set { this.Set<GrayscaleMode>("s.grayscale", value);  } }
      #
      # /// <summary>
      # /// Value between 0 and 1. Makes the rendered image transparent. Does not affect borders or background colors - those accept 4-byte colors with alpha channels, however.
      # /// Requires the SimpleFilters plugin. Unless the output format is PNG, the image will be blended against white or the background color.
      # /// </summary>
      # public double? Alpha { get { return this.Get<double>("s.alpha"); } set { this.Set<double>("s.alpha", value); } }
      #
      # /// <summary>
      # /// -1..1 Adjust the brightness of the image. Requires the SimpleFilters plugin
      # /// </summary>
      # public double? Brightness { get { return this.Get<double>("s.brightness"); } set { this.Set<double>("s.brightness", value); } }
      # /// <summary>
      # /// -1..1 Adjust the contrast of the image. Requires the SimpleFilters plugin
      # /// </summary>
      # public double? Contrast { get { return this.Get<double>("s.contrast"); } set { this.Set<double>("s.contrast", value); } }
      # /// <summary>
      # /// -1..1 Adjust the saturation of the image. Requires the SimpleFilters plugin
      # /// </summary>
      # public double? Saturation { get { return this.Get<double>("s.saturation"); } set { this.Set<double>("s.saturation", value); } }
      #
      # /// <summary>
      # /// Setting this enables automatic whitespace trimming using an energy function. 50 is safe, even 255 rarely cuts anything off except a shadow. Set TrimPadding to pad the result slightly and improve appearance.
      # /// Requires the WhitespaceTrimmer plugin.
      # /// </summary>
      # public byte? TrimThreshold { get { return this.Get<byte>("trim.threshold"); } set { this.Set<byte>("trim.threshold", value); } }
      # /// <summary>
      # /// Set TrimThreshold first. This specifies a percentage of the image size to 'add' to the crop rectangle. Setting to 0.5 or 1 usually produces good results.
      # /// Requires the WhitespaceTrimmer plugin.
      # /// </summary>
      # public double? TrimPadding { get { return this.Get<double>("trim.percentpadding"); } set { this.Set<double>("trim.percentpadding", value); } }
      #
      # /// <summary>
      # /// Guassian Blur. Requires the AdvancedFilters plugin.
      # /// </summary>
      # public double? Blur { get { return this.Get<double>("a.blur"); } set { this.Set<double>("a.blur", value); } }
      #
      # /// <summary>
      # /// Unsharp Mask. Requires the AdvancedFilters plugin.
      # /// </summary>
      # public double? Sharpen { get { return this.Get<double>("a.sharpen"); } set { this.Set<double>("a.sharpen", value); } }
      #
      # /// <summary>
      # /// Safe noise removal. Requires the AdvancedFilters plugin.
      # /// </summary>
      # public double? RemoveNoise { get { return this.Get<double>("a.removenoise"); } set { this.Set<double>("a.removenoise", value); } }
      #
      # /// <summary>
      # /// Controls dithering when rendering to an 8-bit PNG or GIF image. Requires PrettyGifs or WicEncoder. Accepted values for PrettyGifs: true|false|4pass|30|50|79|[percentage]. Accepted values for WicEncoder: true|false.
      # /// </summary>
      # public string Dither { get { return this["dither"]; } set { this["dither"] = value; } }
      #
      #
      # /// <summary>
      # /// Specify a preferred encoder for compressing the output image file. Defaults to 'gdi'. Other valid values are 'freeimage' and 'wic', which require the FreeImageEncoder and WicEncoder plugins respectively.
      # /// FreeImage offers faster jpeg encoding, while WIC offers faster PNG and GIF encoding. Both, however, require full trust.
      # /// </summary>
      # public string Encoder { get { return this["encoder"]; } set { this["encoder"] = value; } }
      #
      # /// <summary>
      # /// Specify a preferred decoder for parsing the original image file. Defaults to 'gdi'. Other values include 'freeimage', 'wic', and 'psdreader'. The preferred decoder gets the first chance at reading the files. If that fails, all other decoders try, in order of declaration in Web.config.
      # /// Requires the matching FreeImageDecoder, WicDecoder, or PsdReader plugin to be installed.
      # /// </summary>
      # public string Decoder { get { return this["decoder"]; } set { this["decoder"] = value; } }
      #
      # /// <summary>
      # /// Specify the image processing pipeline to use. Defaults to 'gdi'. If FreeImageBuilder or WicBuilder is installed, you can specify 'freeimage' or 'wic' to use that pipeline instead.
      # /// The WIC pipeline offers a 2-8X performance increase of GDI, at the expense of slightly reduced image quality, the full trust requirement, and support for only basic resize and crop commands.
      # /// FreeImage offers *nix-level image support, and handles many images that gdi and wic can't deal with. It is also restricted to a subset of the full command series.
      # /// </summary>
      # public string Builder { get { return this["builder"]; } set { this["builder"] = value; } }
      #
      # /// <summary>
      # /// Gets or sets a 1 or 4-element array defining cornder radii. If the array is 1 element, it applies to all corners. If it is 4 elements, each corner gets an individual radius. Values are percentages of the image width or height, whichever is smaller.
      # 		/// Requires the SimpleFilters plugin.
      # /// </summary>
      # public double[] RoundCorners { get { return this.GetList<double>( "s.roundcorners", 0, 4, 1); } set { this.SetList("s.roundcorners", value, true, 4, 1); } }
      #
      #
      #
      #
      # /// <summary>
      # /// ["paddingWidth"]: Gets/sets the width(s) of padding inside the image border.
      # /// </summary>
      # public BoxEdges Padding {
      # get {
      # return BoxEdges.Parse(this["paddingWidth"],null);
      # }
      # set {
      # this.SetAsString<BoxEdges>("paddingWidth", value);
      # }
      # }
      # /// <summary>
      # /// ["margin"]: Gets/sets the width(s) of the margin outside the image border and effects.
      # /// </summary>
      # public BoxEdges Margin {
      # get {
      # return BoxEdges.Parse(this["margin"],null);
      # }
      # set {
      # this.SetAsString<BoxEdges>("margin", value);
      # }
      # }
      # /// <summary>
      # /// Friendly get/set accessor for the ["borderWidth"] value. Returns null when unspecified.
      # /// </summary>
      # public BoxEdges Border {
      # get {
      # return BoxEdges.Parse(this["borderWidth"], null);
      # }
      # set {
      # this.SetAsString<BoxEdges>("borderWidth", value);
      # }
      # }


    end
  end
end
