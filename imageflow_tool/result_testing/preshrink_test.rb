#!/usr/bin/env ruby

`convert --version`
`./imageflow_tool --version`

`./fetch_images.sh`

`mkdir -p preshrink`
EXEPATH = "./"
IMAGEPATH = "./source_images/"
OUTPATH ="./preshrink/"

SIZES = [250, 400, 800, 1600]

IMAGES = ["turtleegglarge.jpg","rings2.png","gamma_dalai_lama_gray.jpg","u1.jpg","waterhouse.jpg","u6.jpg","premult_test.png","gamma_test.jpg"]
IMAGES=["u1_square.jpg"]
COLORSPACES = ["srgb", "linear"]
FILTERS = ["catmull_rom"]
ALTERNATE_TOOLS = ["flow_legacy_idct","flow_catrom_idct"]
RESULTS = []

IMAGES.each do |image|
    COLORSPACES.each do |colorspace|
        FILTERS.each do |filter|
            SIZES.each do |size|
                ALTERNATE_TOOLS.each do |tool|
                    command = " v1/querystring --command=\"w=#{size}&down.filter=#{filter}&down.colorspace=#{colorspace}&format=png"
                    reference_path = "./preshrink/#{image}_#{size}w_#{filter}_#{colorspace}.png"
                    preshrink_path =  "./preshrink/preshrink_#{tool}_#{image}_#{size}w_#{filter}_#{colorspace}.png"
                    reference_command = "./imageflow_tool #{command}&decoder.min_precise_scaling_ratio=100\" --in ./source_images/#{image} --out #{reference_path} --quiet"
                    puts "\nRef: #{reference_command}\n"
                    `#{reference_command}`
                    preshrink_command = "./#{tool} #{command}&decoder.min_precise_scaling_ratio=1\" --in ./source_images/#{image} --out #{preshrink_path} --quiet"
                    puts "\nPreshrink: #{preshrink_command}\n"
                    `#{preshrink_command}`
                    dssim = `dssim #{reference_path} #{preshrink_path}`
                    RESULTS << {dssim: dssim, dssim_value: dssim.strip.to_f, path: preshrink_path, reference_path: reference_path}
                end
            end
        end
    end
end

compares = []
puts "\n---------------------\n"
RESULTS.select{|w| !w[:dssim_value].nil? && w[:dssim_value] > 0 }.sort_by{|w| w[:dssim_value]}.reverse.take(100).each do |w|
  puts "#{w[:dssim].strip}\n"
  compares << "#{w[:dssim].strip}\ncompare #{w[:path]} #{w[:reference_path]} x:\n"
end

puts "----------------------\n"
compares.each do |c|
    puts c
end