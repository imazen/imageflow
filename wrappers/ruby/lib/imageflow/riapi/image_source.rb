module Imageflow
  module Riapi
    class ImageSource

      attr_reader :type, :resource_identifier, :bytes

      def initialize(type:, resource_identifier:, bytes: nil)
        @type = type
        @resource_identifier = resource_identifier
        @bytes = bytes
      end

      def self.from_file(path:)
        ImageSource.new resource_identifier: path, type: :file
      end

      def self.from_binary(binary_string:, optional_resource_identifier: "")
        ImageSource.new resource_identifier: optional_resource_identifier, type: :binary, bytes: binary_string
      end

      def self.from_url(url:)
        ImageSource.new resource_identifier: url, type: :url
      end


      def load_bytes!
        return unless @bytes.nil?
        if type == :url
          @bytes =Net::HTTP.get(URI(resource_identifier))
        elsif type == :file
          @bytes = File.open(resource_identifier, "rb") { |f| f.read }
        else
          raise "Unsupported resource type #{type}"
        end
      end


    end
  end
end
