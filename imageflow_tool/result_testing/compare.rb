#!/usr/bin/env ruby

IMAGES = ["turtleegglarge.jpg","rings2.png","gamma_dalai_lama_gray.jpg","u1.jpg", "u1_square.jpg", "waterhouse.jpg","u6.jpg","premult_test.png","gamma_test.jpg"]


version_info = `convert --version` + `./imageflow_tool --version` + `./imagew --version`

`./fetch_images.sh`

`mkdir -p ./compare/images`
EXEPATH = "./"
IMAGEPATH = "./source_images/"
OUTPATH ="./compare/images/"
IMAGERELPATH="images/"

def create_command(tool, image, nogamma, filter, sharpen, w)
  basename = File.basename(image)
  sharpen ||= "0"
  filter ||= "robidoux"
  infile = "#{IMAGEPATH}#{image}"
  outfile = "#{basename}_#{tool}_#{nogamma ? 'nogamma' : 'linear'}_#{filter}_s#{sharpen}_w#{w}.png"
  if tool == :imagew
    w_filter = "-filter #{filter}"
    if filter == :robidoux
      w_filter = " -filter cubic0.37821575509399867,0.31089212245300067 "
    end
    if filter == :robidoux_sharp
      w_filter=" -filter cubic0.2620145123990142,0.3689927438004929 "
    end
    if filter == :n_cubic
      w_filter=" -filter cubic0.37821575509399867,0.31089212245300067 -blur 0.85574108326"
    end
    if filter == :n_cubic_sharp
      w_filter=" -filter cubic0.2620145123990142,0.3689927438004929 -blur 0.90430390753 "
    end
    if filter == :catmull_rom
      w_filter = " -filter catrom "
    end
    if filter == :cubic_b_spline
      w_filter = " -filter bspline "
    end
    if filter == :ginseng
      return nil #Because imagew doesn't do ginseng
    end

    command = "#{EXEPATH}imagew #{nogamma ? '-nogamma' : ''} #{w_filter} #{infile} -w #{w} #{OUTPATH}#{outfile}"
  end
  if tool == :magick
    magick_filter = "-filter #{filter}"
    if filter == :ginseng
      magick_filter = " -define filter:filter=Sinc -define filter:window=Jinc -define filter:lobes=3 "
    end
    if filter == :n_cubic
      magick_filter = " -filter robidoux -define filter:blur=0.85574108326 "
    end
    if filter == :cubic_b_spline
      magick_filter = " -filter spline"
    end
    if filter == :n_cubic_sharp
      magick_filter = " -filter robidouxsharp -define filter:blur=0.90430390753  "
    end
    if filter == :catmull_rom
      magick_filter = " -filter catrom "
    end
    if filter == :robidoux_sharp
      magick_filter = " -filter robidouxsharp "
    end
    if nogamma
      command = "convert #{infile} #{magick_filter} -resize #{w} #{OUTPATH}#{outfile}"
    else
      command = "convert #{infile} -set colorspace sRGB -colorspace RGB #{magick_filter}  -resize #{w} -colorspace sRGB #{OUTPATH}#{outfile}"
    end
  end
  if tool == :flow || tool == :flow_preshrink
    outputformat = image =~ /\.png/ ? "png" : "png"
    command = "#{EXEPATH}imageflow_tool v1/querystring --command=\"&f.sharpen=#{sharpen}&w=#{w}&down.filter=#{filter}#{nogamma ? '&down.colorspace=srgb&up.colorspace=srgb' : '&down.colorspace=linear&up.colorspace=linear'}&decoder.min_precise_scaling_ratio=#{tool == :flow ? 100 : 1}&up.filter=#{filter}&format=#{outputformat}&scale=both\" --in #{infile} --out #{OUTPATH}#{outfile} --quiet"
  end
  {command: command, image: image, gamma: nogamma ? 'nogamma' : 'linear', filter: filter, sharpen: sharpen,
    w: w, tool: tool,
    relpath: "#{IMAGERELPATH}#{outfile}", path:  "#{OUTPATH}#{outfile}"}
end

# imageflow's box filter acts differently, and is not compared here
SIZES = [200,400,800]


FILTERS = [:triangle, :lanczos, :lanczos2, :ginseng, :n_cubic, :n_cubic_sharp, :robidoux, :robidoux_sharp,
:cubic_b_spline, :hermite, :catmull_rom, :mitchell]
GAMMAS = [true, false]
SHARPENS = [0]

def generate_for(tool)
  commands = []
  IMAGES.each do |img|
    SIZES.each do |width|
      FILTERS.each do |filter|
        GAMMAS.each do |gamma|
          (tool == :flow ? SHARPENS : [0]).each do |sharpen|
            commands << create_command(tool, img, gamma, filter, sharpen, width)
          end
        end
      end
    end
  end
  commands.compact
end

tools = [:flow, :flow_preshrink, :magick, :imagew]

require 'thread'

flow_commands = generate_for(:flow) + generate_for(:flow_preshrink)

commands = generate_for(:imagew) + generate_for(:magick) + flow_commands

queue = Queue.new
commands.each {|c| queue << c}

completed_work = []

Thread.abort_on_exception = true

consumers = (0..5).map do |t|
  Thread.new do
    while !queue.empty? do
      work = queue.pop
      unless File.exist? work[:path]
        begin
            work[:output] = `#{work[:command]}`
        rescue => e
            puts "Failed to run #{work[:command]}: #{e}"
            raise
        end
      end
      completed_work << work
    end
  end
end

consumers.each{ |c| c.join }

flow_commands.each {|c| queue << c}

consumers = (0..5).map do |t|
  Thread.new do
    while !queue.empty? do
      work = queue.pop
      compare_to_path = work[:path].gsub(/_flow_preshrink_/,"_imagew_").gsub(/_flow_/,"_imagew_")
      next unless File.exist? compare_to_path

      dssim_path = "#{work[:path]}_dssim.txt"
      if File.exist? dssim_path
        work[:dssim] = IO.read(dssim_path)
      else
        work[:dssim] = `dssim #{work[:path]} #{compare_to_path}`
        if $?.exitstatus == 0
          IO.write(dssim_path, work[:dssim]) 
        else
          puts `identify #{work[:path]}`
          puts `identify #{compare_to_path}`
        end 

      end
      work[:dssim_value] = work[:dssim].strip.to_f
    end
  end
end
consumers.each{ |c| c.join }

completed_work.select{|w| !w[:dssim_value].nil? && w[:dssim_value] > 0 }.sort_by{|w| w[:dssim_value]}.reverse.take(100).each do |w|
  puts "#{w[:dssim].strip}: #{w[:relpath]}\n"
end


#TODO: run dssim/compare

require 'json'

json_hash = {info: version_info, images: completed_work, image_names: IMAGES, widths: SIZES,
tools: tools, filters: FILTERS, sharpen_values: SHARPENS, gamma_values: ["nogamma", "linear"]}

IO.write("./compare/data.js", "window.data = " + JSON.pretty_generate(json_hash) + ";")

puts "\n Open ./compare/compare.html in your browser.\n"
puts "open ./compare/compare.html  || x-www-browser ./compare/compare.html "