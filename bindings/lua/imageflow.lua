local ffi_def = require("imageflow_ffi")
local f = ffi_def.load("imageflow")


c = f.imageflow_context_create()

f.imageflow_destroy(c)
