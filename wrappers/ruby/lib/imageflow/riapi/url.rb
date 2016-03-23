module Imageflow
  module Riapi
    module Url
      def self.build_query_string(hash_of_strings:, url_encode: true, skip_null_values: true, first_separator: "?", later_separators: "&", equals: "=")
        h = hash_of_strings

        query = h.select { |k, v| !(k.nil? || (skip_null_values && v.nil?)) }.map do |k, v|
          !url_encode ? "#{k.to_s}#{equals}#{v.to_s}" : "#{Rack::Utils.escape_path(k.to_s)}#{equals}#{Rack::Utils.escape_path(v.to_s)}"
        end.join(later_separators)

        query.length > 0 ? first_separator + query : query
      end


      def self.add_implicit_questionmark(path, allow_semicolons: true)
        sub_path = path.gsub(/#.*$/, "")
        eq = sub_path.index '='
        quest = sub_path.index '?'
        semi = allow_semicolons ? sub_path.index(';') : nil
        path = "?#{path}" if !eq.nil? && quest.nil? && (semi.nil? || eq < semi)
        path
      end

      #Like ParseQueryString, but permits the leading '?' to be omitted, and semicolons can be substituted for '&amp;'
      def self.parse_query_string_implicit(path, allow_semicolons: false, path_segments: {})
        path = add_implicit_questionmark(path, allow_semicolons: allow_semicolons)
        parse_query_string(path, allow_semicolons: allow_semicolons, path_segments: {})
      end


      # parses the querystring from the given path
      #   accepts "file?key=value" and "?key=value&amp;key2=value2" formats. (no path is required)
      # UrlDecodes keys and values. Does not enforce correct syntax, I.E. '?key=value?key2=value2' is allowed. However, '&amp;key=value?key2=value' will only get key2 parsed.
      #When allowSemicolons is true, semicolon paths like ';key=value;key2=value2' are allowed, as are hybrid paths: ';key=value?key2=value2&amp;key3=value3'.
      # Does NOT parse fragments correctly.
      #stores fragment and pre-querystring segment in path_segments
      def self.parse_query_string(path, allow_semicolons: false, path_segments: {})
        frag = path.index '#'
        path_segments[:fragment] = frag.nil? ? "" : path[frag..-1]
        path = path[0...frag] unless frag.nil?

        delim = allow_semicolons ? [path.index('?'), path.index(';')].compact.min : path.index('?')

        path_segments[:before_query] = path[0..(delim || 0) - 1]

        query = delim.nil? ? "" : path[delim..-1]

        parse_query_only(query, allow_semicolons: allow_semicolons)

      end

      #Parses a querystring into a name/value collection. The given string cannot include path or fragment information - it must be *just* the querystring.
      def self.parse_query_only(path, allow_semicolons: true, url_decode: true)
        hash = {}
        path.split(allow_semicolons ? /[\?\&\;]/ : /[\?\&]/).compact.select { |s| s.length > 0 }.each do |pair_str|
          first_eq = pair_str.index '='
          k = first_eq.nil? ? pair_str : pair_str[0...first_eq]
          v = first_eq.nil? ? "" : pair_str[first_eq + 1.. -1] || ""
          if url_decode
            k = Rack::Utils.unescape(k)
            v = Rack::Utils.unescape(v)
          end
          hash[k] = v
        end
        hash
      end

    end
  end
end

