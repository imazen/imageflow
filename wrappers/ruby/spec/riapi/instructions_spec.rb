require 'rspec'
require 'imageflow'
require 'imageflow/riapi'
module Imageflow::Riapi

  describe 'imageflow' do

    describe 'Instructions' do

      describe '#initialize' do
        it "can be created with a hash" do
          i = Instructions.new({"a" => "b"})
          expect(i.to_s).to eq(";a=b")
        end
        it "can be created with a string" do
          i = Instructions.new("?a=b")
          expect(i.to_s).to eq(";a=b")
        end
      end

      describe "#[]" do
        it "can lookup case-insensitive" do
          i = Instructions.new({"a" => "b"})
          expect(i["A"]).to eq("b")
        end
        it "can lookup by symbols case-insensitive" do
          i = Instructions.new({"a" => "b"})
          expect(i[:"A"]).to eq("b")
        end
        it "can lookup symbols by case-insensitive strings" do
          i = Instructions.new({a: "b"})
          expect(i["A"]).to eq("b")
        end
      end

      describe "#parse_enum" do
        it "returns nil if value unrecognized" do
          expect(Instructions.parse_enum("hello", map: {"nope" => :value})).to be_nil
        end

        it "returns nil if nil" do
          expect(Instructions.parse_enum(nil, map: {"nope" => :value})).to be_nil
        end

        it "is case insensitive" do
          expect(Instructions.parse_enum("HEY", map: {"hey" => :value})).to be(:value)
        end
      end

      describe "#stringify_enum" do
        it "leaves nil as-is" do
          expect(Instructions.stringify_enum(nil, values: [])).to be_nil
        end
        it "removes underscores" do
          expect(Instructions.stringify_enum(:this_or__that, values: [:this_or__that])).to eq("thisorthat")
        end
        it "raises and exception if not valid" do
          expect {
            Instructions.stringify_enum(:unknown, values: [:a, :b])
          }.to raise_error /Value must be/
        end
      end

      describe "#parse_bool" do
        it "leaves nil as-is" do
          expect(Instructions.parse_bool(nil)).to be_nil
        end
        it 'returns nil for non-boolean strings' do
          expect(Instructions.parse_bool("hey!")).to be_nil
        end

        it 'parses valid values' do
          ["YES", "On", "1", "TRUE", "true"].each do |s|
            expect(Instructions.parse_bool(s)).to eq(true)
          end
          ["no", "OFF", "0", "FALse", "false"].each do |s|
            expect(Instructions.parse_bool(s)).to eq(false)
          end
        end
      end


      describe "#parse_list" do
        it "leaves nil as-is" do
          expect(Instructions.parse_list(nil)).to be_nil
        end
        it 'returns nil if an invalid qty of values is present' do
          expect(Instructions.parse_list("a,b,c", default_element: nil, permitted_counts: [1, 2])).to be_nil
        end
        it 'returns nil if any values are missing and default_element is nil' do
          expect(Instructions.parse_list("a,,c", default_element: nil, permitted_counts: [3])).to be_nil
        end

        it 'substitutes default_element for missing items' do
          expect(Instructions.parse_list("a,,c", default_element: "b", permitted_counts: [3])).to eq(["a", "b", "c"])
        end

        it 'substitutes default_element for items which fail parsing' do
          expect(Instructions.parse_list("1,,c", default_element: 2, value_type: :decimal, permitted_counts: [3])).to eq([1, 2, 2])
        end

        it 'parses decimals' do
          expect(Instructions.parse_list("1.2,3.1,5.1", default_element: nil, value_type: :decimal, permitted_counts: [3])).to eq([1.2, 3.1, 5.1])
        end

        it 'ignores trailing/leading spaces and parens' do
          expect(Instructions.parse_list("  (1.2,3.1,5.1 )", default_element: nil, value_type: :decimal, permitted_counts: [3])).to eq([1.2, 3.1, 5.1])
        end


      end

      describe "#normalize" do
        it 'normalizes a messed up hash' do
          i = Instructions.new({:a => 1, "a" => 2, "A" => 3, "b" => 4})
          i.normalize(keys: ["A", :b])
          expect(i.hash).to eq({"A" => 1})
        end

      end

      describe "#get_first" do
        it 'grabs by preference' do
          i = Instructions.new({:b => 2, :a => 1})
          expect(i.get_first(keys: [:a, :b])).to eq(1)
        end
      end

      describe "#set_first" do
        it 'deletes if passed nil' do
          i = Instructions.new({:a => 1})
          i.set_first(keys: "a", value: nil)
          expect(i.hash).to eq({})
        end
      end

      describe "#clamp_decimal" do
        it 'clamps' do
          expect(Instructions.clamp_decimal(3, min: 5, max: nil)).to eq(5)
          expect(Instructions.clamp_decimal(3, min: 2, max: nil)).to eq(3)
          expect(Instructions.clamp_decimal(3, min: 1, max: 2)).to eq(2)
          expect(Instructions.clamp_decimal(3, min: 1, max: 4)).to eq(3)
          expect(Instructions.clamp_decimal(3, min: nil, max: 4)).to eq(3)
          expect(Instructions.clamp_decimal(3, min: nil, max: nil)).to eq(3)
        end
      end


      describe "#width" do
        it "should be readable and writable" do
          i = Instructions.new "?w=200&width=300&WIDTH=400"
          expect(i.width).to eq(300)
          i.width = 50
          expect(i.to_s).to eq(";width=50")
        end
      end


      describe "#crop_array" do
        it "should be readable and writable" do
          i = Instructions.new "?crop=(1,2,-1,-1)"
          expect(i.crop_array).to eq([1, 2, -1, -1])
          i.crop_array = [0.2, 0.2, 0.8, 0.8]
          expect(i.to_s).to eq(";crop=0.2,0.2,0.8,0.8")
        end
      end

      describe "#scale" do
        it "should be readable and writable" do
          i = Instructions.new "?scale=up"
          expect(i.scale).to eq(:upscale_only)
          i.scale = :downscale_only
          expect(i.to_s).to eq(";scale=downscaleonly")
        end
      end

      describe "#zoom" do
        it "should be readable and writable" do
          i = Instructions.new "?zoom=2.3"
          expect(i.zoom).to eq(2.3)
          i.zoom = 5.3
          expect(i.to_s).to eq(";zoom=5.3")
        end
      end
      describe "#autorotate" do
        it "should be readable and writable" do
          i = Instructions.new "?autorotate=true"
          expect(i.autorotate).to eq(true)
          i.autorotate = false
          expect(i.to_s).to eq(";autorotate=false")
        end
      end
      describe "#preset" do
        it "should be readable and writable" do
          i = Instructions.new "?preset=1"
          expect(i.preset).to eq("1")
          i.preset = "thumb"
          expect(i.to_s).to eq(";preset=thumb")
        end
      end
    end
  end
end
