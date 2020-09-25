# Comparing output

In this folder, we have a variety of systems for automatically and visually comparing output from flow-proto1, imageworsener, and imagemagick. 

Run `./install_tools.sh` to install everything except imagemagick (for that, you're basically on your own - but check /magick.md in the root).


## Scope.sh

This uses resamplescope to plot the resampling functions of imageflow and imageworsener

run ./scope.sh

When complete, the folder `./scope` will be opened for you to compare the plots.

## Compare.rb

This system will test 8 different images, scaled to 3 different sizes, with 11 different filters, with and without gamma correction, and compare the results with `imagemagick`, `imageworsener`, and `flow-proto`, both using DSSIM and by generating an interactive web page for subjective analysis. 

The most dissimilar files will be listed first.

Open ./compare/index.html in your browser when complete. 

## Updating Imageflow results

1.  Run `./compare_reset_flow_images.sh` to invalidate all imageflow-generated results. 
2. delete imageflow_tool
3. Run `./install_tools.sh`
4. Run  `./compare.rb`
