# -*- encoding: utf-8 -*-
lib = File.expand_path('../lib/', __FILE__)
$:.unshift lib unless $:.include?(lib)

require 'imageflow/version'

Gem::Specification.new do |s|
  s.name        = "bundler"
  s.version     = Imageflow::VERSION
  s.platform    = Gem::Platform::RUBY
  s.summary     = "Wrapper for libimageflow"
  s.description = "Process images quickly and correctly for the  web"
  s.authors     = ["Nathanael Jones"]
  s.email       = ["support@imageresizing.net"]
  s.homepage    = "http://github.com/imazen/imageflow"
  s.required_rubygems_version = ">= 1.3.6"
  #s.rubyforge_project         = "bundler"


  s.files        = Dir.glob("{bin,lib}/**/*") #+ %w(LICENSE README.md)
  s.test_files        = Dir.glob("{test/spec}/**/*")
  #s.executables  = ['bundle']
  s.require_path = 'lib'
  s.license = "AGPL v3"




  s.add_development_dependency "rspec"
  s.add_development_dependency "sinatra"
  s.add_development_dependency "pry"
  s.add_development_dependency "rake"
  s.add_development_dependency "ffi-swig-generator"

  s.add_dependency 'ffi', '>=1.9.10'

  s.add_development_dependency 'ffi', '>=1.9.10'
end

