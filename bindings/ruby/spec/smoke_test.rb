# frozen_string_literal: true

# This is a smoke test to ensure the generated Ruby code is loadable.
# It attempts to require the main library file.

begin
  require 'imageflow'
  
  puts 'Smoke test passed: Successfully loaded the generated Imageflow Ruby client.'
rescue LoadError => e
  puts "Smoke test failed: Could not load the generated library. Error: #{e.message}"
  exit 1
end
