# -*- encoding: utf-8 -*-
lib = File.expand_path('../lib/', __FILE__)
$:.unshift lib unless $:.include?(lib)

require 'bundler/version'

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

  s.add_dependency('ffi')

  s.add_development_dependency "rspec"
  s.add_development_dependency "sinatra"
  s.add_development_dependency "pry"
  s.add_development_dependency "rake"

  s.files        = Dir.glob("{bin,lib}/**/*") + %w(LICENSE README.md)
  #s.executables  = ['bundle']
  s.require_path = 'lib'
  s.license = "AGPL v3"
end

