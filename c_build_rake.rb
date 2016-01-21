require 'rake/loaders/makefile'
require 'rbconfig'
Rake.application.add_loader('d', Rake::MakefileLoader.new)

CLEAN = Rake::FileList["**/*~", "**/*.bak", "**/core"]
CLEAN.clear_exclude.exclude { |fn|
  fn.pathmap("%f").downcase == 'core' && File.directory?(fn)
}

MACOS = !!(/darwin|mac os/ =~ RbConfig::CONFIG['host_os'])

GCC = MACOS ? "gcc-4.9" : "gcc"
GPP = MACOS ? "g++-4.9" : "g++"
CXX = ENV['CXX'] || GPP
CC = ENV['CC'] || GCC
VALGRIND_OPTS =  MACOS ? "--dsymutil=yes --error-exitcode=2" : "--error-exitcode=2"


EXTRA_CFLAGS=ENV['EXTRA_CFLAGS']


desc "Remove any temporary products."
task :clean do
  CLEAN.each do |fn|
    rm_rf fn
    if Rake::Task.task_defined?(fn)
      task = Rake::Task[fn]
      task.reenable
    end
  end
end

CLOBBER = Rake::FileList.new

desc "Remove any generated file."
task :clobber => [:clean] do
  CLOBBER.each { |fn| rm_r fn rescue nil }
end


class String
  def to_task
    Rake::Task[self]
  end
end

rule '.o' => '.cpp' do |t|
  sh "#{CXX}  #{CXXFLAGS} #{EXTRA_CFLAGS} -MMD -c -o #{t.name} #{t.source}"
end

rule '.o' => '.c' do |t|
  sh "#{CC}  #{CFLAGS} #{EXTRA_CFLAGS} -MMD -c -o #{t.name} #{t.source}"
end


rule '.d' => '.cpp' do |t|
  verbose(false) do 
    sh "#{CXX} -MM -MG -MF #{t.name} #{t.source}"
  end
end

rule '.d' => '.c' do |t|
  verbose(false) do 
    sh "#{CC} -MM -MG -MF #{t.name} #{t.source}"
  end
end

def register_objects(object_files)
  object_files.each { |object| CLEAN << object }
  dependency_files = object_files.map { |object_file| object_file.pathmap("%X.d") }
  dependency_files.each do |dependency_file|
    CLEAN << dependency_file
    dependency_file.to_task
    import dependency_file
  end
end



