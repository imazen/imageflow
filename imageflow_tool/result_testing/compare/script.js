
function fill_dd(dd, list){
  $.each(list, function(ix, val) {
    dd.append( new Option(val,val) );
  });
}
$(function(){
  var list_img = $('#image');
  var list_w = $('#width');
  var list_gamma = $('#gamma');
  var list_filter = $('#filter');
  var list_sharpen = $('#sharpen');

  var list_baseline_tool = $('#baselinetool');
  var list_tool = $('#tool');
  var dropdowns = $('.compare-dd');
  var display = $('#cmp');

  var images = window.data.image_names; //['premult_test.png', 'gamma_test.jpg', 'turtleegglarge.jpg', 'u1.jpg', 'u6.jpg','rings2.png' ];
  fill_dd(list_img, images);

  var gammas = window.data.gamma_values; //['linear', 'nogamma'];
  fill_dd(list_gamma, gammas);

  var sizes = window.data.widths;//[200,400,800];
  fill_dd(list_w, sizes);
  
  fill_dd(list_tool, ['flow', 'magick','imagew', 'flow_preshrink' ]);
  fill_dd(list_baseline_tool, ['imagew','flow', 'magick', 'flow_preshrink']);
  
  var filters = window.data.filters;//['ncubic' , 'ncubicsharp' , 'robidoux' , 'robidouxsharp' , 'ginseng' , 'lanczos' , 'lanczos2' ,
  //'box' , 'triangle' , 'bspline' , 'hermite' , 'catrom' , 'mitchell'];
  fill_dd(list_filter, filters);
  
  var sharpens = window.data.sharpen_values;//[0,2,5,10];
  fill_dd(list_sharpen, sharpens);

  var find_match = function(tool){
    for (var i =0; i < window.data.images.length; i++){
       var elem = window.data.images[i];
       if (elem.image == list_img.val() && 
             elem.w == list_w.val() &&
             elem.filter == list_filter.val() &&
             elem.sharpen == list_sharpen.val() &&
             elem.gamma == list_gamma.val() && 
             elem.tool == tool
            ){
          return elem;
       }
    }
    return {relpath: "", tool: "", command:"not generated"};
  };

  var statustext = $('#status');
  var outputtext = $('#commandoutput');
  var b_link = $('#baseline_link');
  var h_link = $('#hover_link');

  var dssim = $('#dssim');
  var diff_them = $('#diff');
  var update_ui = function(hovering){
    var imgdata = hovering ? window.image_hover : window.image_baseline;
    b_link.attr('href', window.image_baseline.relpath);
    h_link.attr('href', window.image_hover.relpath);
    b_link.text(window.image_baseline.relpath);
    h_link.text(window.image_hover.relpath);
    dssim.val(imgdata.dssim);
    diff_them.val("compare " + window.image_baseline.path + " -fuzz 0.5% " + window.image_hover.path + " x:");
    display.attr('src',imgdata.relpath);
    display.attr('width', (imgdata.w / window.devicePixelRatio) + "px" );
    statustext.val(imgdata.tool + ": using " + imgdata.command);
    outputtext.text(imgdata.output);
  };

  var update_pair = function(){
    window.image_baseline = find_match(list_baseline_tool.val());
    window.image_hover = find_match(list_tool.val());
    
    update_ui(false);
  };
  
  dropdowns.change(function(){
    update_pair();
  });
  display.mouseover(function(){
    update_ui(true);
  }).mouseout(function(){
    update_ui(false);
  });
  update_pair();


});

