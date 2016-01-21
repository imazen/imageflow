require_relative 'c_build_rake'

task "malloc.h" do end
  
task :default => "fastscaling"

%w{ test_program fastscaling libfastscaling.so }.each { |file| CLOBBER.include(file) if File.exists?(file) }


TRAVIS_USAFE_FLAGS = " -Wfloat-conversion "

COMMON_FLAGS="-Iinclude -iquotelib -fPIC -O2 -g -Wpointer-arith -Wcast-qual -Wpedantic -Wall -Wextra -Wno-unused-parameter -Wuninitialized -Wredundant-decls -Werror"

CFLAGS=ENV.fetch("CFLAGS", "#{COMMON_FLAGS} #{ENV['CI'] ? '' : TRAVIS_USAFE_FLAGS} -std=gnu11 -Wstrict-prototypes -Wmissing-prototypes -Wc++-compat -Wshadow")

CXXFLAGS=ENV.fetch("CXXFLAGS", "#{COMMON_FLAGS} #{ENV['CI'] ? '' : TRAVIS_USAFE_FLAGS} -std=gnu++11")


LIB_OBJECTS = FileList[File.absolute_path('lib/*.c')].ext('.o')
SRC_OBJECTS = FileList[File.absolute_path('src/*.c')].ext('.o')
TEST_OBJECTS = FileList[File.absolute_path('tests/*.cpp')].ext('.o')
THEFT_TEST_OBJECTS = FileList[File.absolute_path('theft_tests/*.cpp')].ext('o')

SO_FILE="libfastscaling.so"
TEST_PROGRAM = "test_program"
THEFT_TEST_PROGRAM = "theft_test"
PROFILING_PROGRAM = "fastscaling"

desc "build a fastscaling program"
file PROFILING_PROGRAM => SRC_OBJECTS + LIB_OBJECTS  do |t|
  sh "#{CC} -o #{t.name} -Werror #{t.prerequisites.join(" ")} -lm"
end


desc "build the fastscaling library"
file SO_FILE => LIB_OBJECTS do |t|
  sh "#{CC}  --shared -o #{t.name} #{t.prerequisites.join(' ')}"
end

def with_ld_library_path(ld_library_path, &block)
  was = ENV['LD_LIBRARY_PATH']
  begin
    ENV['LD_LIBRARY_PATH'] = ld_library_path
    block.call
  ensure
    ENV['LD_LIBRARY_PATH'] = was
  end
end


def valgrind_task(valgrind_params_string, name_and_dependencies)
  task name_and_dependencies do |t|
    with_ld_library_path('.') do 
      sh "valgrind #{VALGRIND_OPTS} #{valgrind_params_string} ./#{t.prerequisites.first} #{ENV['ARGS']}"
    end
  end
end

desc "run with valgrind"
valgrind_task("--leak-check=full --show-leak-kinds=all", :valgrind => PROFILING_PROGRAM)

desc "run with callgrind"
valgrind_task("--tool=callgrind --dump-instr=yes --cache-sim=yes --branch-sim=yes", :callgrind => PROFILING_PROGRAM)

desc "run with cachegrind"
task :cachegrind => PROFILING_PROGRAM do |t|
  cachegrind_out_file = "/tmp/cachegrind-out-file"
  with_ld_library_path('.') do
    sh "valgrind #{VALGRIND_OPTS} --tool=cachegrind --branch-sim=yes --cachegrind-out-file=#{cachegrind_out_file} ./#{t.prerequisites.first}"
  end
  sh "cg_annotate #{cachegrind_out_file}"
end

desc "build the test program"
file TEST_PROGRAM => TEST_OBJECTS + LIB_OBJECTS do |t|
  sh "#{CXX} -Werror #{t.prerequisites.join(" ")} -o #{t.name}"
end

desc "build the theft_test program"
file "theft_test" => THEFT_TEST_OBJECTS + LIB_OBJECTS do |t|
  sh "#{CXX} -Werror #{t.prerequisites.join(" ")} -ltheft -o #{t.name}"
end

task :test => TEST_PROGRAM do
  sh "./#{TEST_PROGRAM} #{ENV['ARGS']}"
end

desc "Run the test program with valgrind"
valgrind_task("--leak-check=full --show-leak-kinds=all --error-exitcode=2", :test_with_valgrind => TEST_PROGRAM)

desc "Run theft_test with valgrind"
valgrind_task("--leak-check=full --show-leak-kinds=all", :theft_test_with_valgrind => "theft_test")

desc "Run the test program with valgrind, (without --dsymutil on mac)"
task :something_else  => TEST_PROGRAM do 
  with_ld_library_path('.') do 
    sh "./#{TEST_PROGRAM} #{ENV['ARGS']}"
  end
end

desc "runs each test case sequentially in a separate process"
task :test_each_case_in_separate_process => "test_program" do
  #use shared library in current directory
  ENV['LD_LIBRARY_PATH'] = "."

  test_name_lines = `./#{TEST_PROGRAM} --list-test-names-only`
  test_names = test_name_lines.split("\n")
  test_names.each do |test_name|
    sh "./#{TEST_PROGRAM} '#{test_name}'"
  end
end

desc "Checked git commit"
task :commit => [:test_with_valgrind, PROFILING_PROGRAM] do
  sh "git add -u"
  sh "git status --porcelain | grep '^?'" do |ok, res|
    raise "Unadded files" if ok
  end
  sh "git commit -v"
end

register_objects(LIB_OBJECTS + TEST_OBJECTS + SRC_OBJECTS + THEFT_TEST_OBJECTS)
