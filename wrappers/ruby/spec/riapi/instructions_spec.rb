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
          expect{
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

      describe "#mode" do
        it "should be readable and writable" do
          i = Instructions.new "?mode=up"
          expect(i.mode).to eq(:upscale_only)
          i.mode = :downscale_only
          expect(i.to_s).to eq(";mode=downscaleonly")
        end
      end

      describe "#zoom" do
        it "should be readable and writable" do
          i = Instructions.new "?zoom=2.3"
          expect(i.zoom).to eq(2.3)
          i.zoom = 5.3
          expect(i.to_s).to eq(";zoom=5.2999999999")
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
