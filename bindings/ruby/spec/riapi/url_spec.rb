require 'rspec'
require 'imageflow'
require 'imageflow/riapi'
module Imageflow::Riapi

  describe 'imageflow' do

    describe 'Uri' do
      it "can build a simple query" do

        result = Url.build_query_string hash_of_strings: {value: true, hi: "hello"}, url_encode: true
        expect(result).to eq("?value=true&hi=hello")
      end

      it "can build a complex query" do

        result = Url.build_query_string hash_of_strings: {'value&': true, hi: "he l///lo"}, url_encode: true
        expect(result).to eq("?value%26=true&hi=he%20l%2F%2F%2Flo")
      end

      describe "#add_implicit_questionmark" do
        it 'works when ignoring semicolons' do
          result = Url.add_implicit_questionmark(";a=b#fragment?=&;", allow_semicolons: false)
          expect(result).to eq("?;a=b#fragment?=&;")

          result = Url.add_implicit_questionmark("a=b#fragment?=&;", allow_semicolons: false)
          expect(result).to eq("?a=b#fragment?=&;")

          result = Url.add_implicit_questionmark("?a=b#fragment?=&;", allow_semicolons: false)
          expect(result).to eq("?a=b#fragment?=&;")
          result = Url.add_implicit_questionmark("b=c?a=b#fragment?=&;", allow_semicolons: false)
          expect(result).to eq("b=c?a=b#fragment?=&;")
        end
        it 'works when respecting semicolons' do
          result = Url.add_implicit_questionmark(";a=b&b=c#fragment?=&;", allow_semicolons: true)
          expect(result).to eq(";a=b&b=c#fragment?=&;")

          result = Url.add_implicit_questionmark(";a=b&b=c", allow_semicolons: true)
          expect(result).to eq(";a=b&b=c")

          result = Url.add_implicit_questionmark("a=b;b=c#fragment?=&;", allow_semicolons: true)
          expect(result).to eq("?a=b;b=c#fragment?=&;")

          result = Url.add_implicit_questionmark("a=b&b=c&d=e", allow_semicolons: true)
          expect(result).to eq("?a=b&b=c&d=e")
        end
      end

      describe "#parse_query_only" do
        it 'can parse malformed queries' do

          result = Url.parse_query_only("?a=b&c==d&j=f=g;l&n&n2=&=1%20", allow_semicolons: false)
          expect(result).to eq({"a" => "b", "c" => "=d", "j" => "f=g;l", "n" => "", "n2" => "", "" => "1 "})

          #TODO: define duplicate query value handling
        end
        it 'can parse query without leading delimiter' do
          result = Url.parse_query_only("a=b", allow_semicolons: false)
          expect(result).to eq({"a" => "b"})
        end

        it 'can parse query with semicolons' do
          result = Url.parse_query_only(";a=b;c=%20d", allow_semicolons: true)
          expect(result).to eq({"a" => "b", "c" => " d"})
        end
      end

      describe "#parse_query_string" do
        it 'can parse query out of path' do
          parts = {}
          result = Url.parse_query_string("http://localhost:200/path.jpg?a=b#frag=2", path_segments: parts, allow_semicolons: true)
          expect(result).to eq({"a" => "b"})
          expect(parts).to eq({fragment: "#frag=2", before_query: "http://localhost:200/path.jpg"})
        end

        it 'cannot parse partial query' do
          parts = {}
          result = Url.parse_query_string("a=b#frag=2", path_segments: parts, allow_semicolons: true)
          expect(result).to eq({})
          expect(parts).to eq({fragment: "#frag=2", before_query: "a=b"})
        end

      end


    end
  end
end
