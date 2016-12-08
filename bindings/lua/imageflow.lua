local ffi_def = require("imageflow_ffi")
local f = ffi_def.load("imageflow")


c = f.imageflow_context_create()

f.imageflow_context_destroy(c)

c2 = f.imageflow_context_create()

f.imageflow_context_destroy(c2)

c3 = f.imageflow_context_create()

f.imageflow_context_destroy(c3)