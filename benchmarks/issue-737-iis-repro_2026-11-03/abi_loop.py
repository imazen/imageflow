"""Drive libimageflow through the same ABI sequence Imageflow.Net 0.15.1 uses
(JobContext + ImageJob.FinishAsync), N times, on a failing input. Reports RSS growth."""
import ctypes as C, sys, os, json

lib_path, img_path, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
qs = sys.argv[4] if len(sys.argv) > 4 else "width=100&mode=max"
L = C.CDLL(lib_path)
P = C.c_void_p
L.imageflow_context_create.restype = P; L.imageflow_context_create.argtypes = [C.c_uint32, C.c_uint32]
L.imageflow_context_add_input_buffer.restype = C.c_bool
L.imageflow_context_add_input_buffer.argtypes = [P, C.c_int32, P, C.c_size_t, C.c_int]
L.imageflow_context_add_output_buffer.restype = C.c_bool; L.imageflow_context_add_output_buffer.argtypes = [P, C.c_int32]
L.imageflow_context_send_json.restype = P; L.imageflow_context_send_json.argtypes = [P, C.c_char_p, P, C.c_size_t]
L.imageflow_context_has_error.restype = C.c_bool; L.imageflow_context_has_error.argtypes = [P]
L.imageflow_context_error_write_to_buffer.restype = C.c_bool
L.imageflow_context_error_write_to_buffer.argtypes = [P, P, C.c_size_t, C.POINTER(C.c_size_t)]
L.imageflow_context_begin_terminate.restype = C.c_bool; L.imageflow_context_begin_terminate.argtypes = [P]
L.imageflow_context_destroy.restype = None; L.imageflow_context_destroy.argtypes = [P]

data = open(img_path, "rb").read()
buf = C.create_string_buffer(data, len(data))  # stays alive (pinned equivalent)
msg = json.dumps({"framewise": {"graph": {"edges": [], "nodes": {"0": {"command_string": {
    "kind": "ir4", "value": qs, "decode": 0, "encode": 1, "watermarks": None}}}}},
    "security": None}, indent=2).encode()
mbuf = C.create_string_buffer(msg, len(msg))

def rss_kb():
    for line in open("/proc/self/status"):
        if line.startswith("VmRSS:"):
            return int(line.split()[1])

first_err = None; begin_term_false = 0; errors = 0; ok = 0
rss0 = rss_kb()
for i in range(n):
    ctx = L.imageflow_context_create(3, 1)
    assert ctx, "context_create returned null"
    assert L.imageflow_context_add_input_buffer(ctx, 0, C.addressof(buf), len(data), 1)
    assert L.imageflow_context_add_output_buffer(ctx, 1)
    L.imageflow_context_send_json(ctx, b"v1/execute\0", C.addressof(mbuf), len(msg))
    if L.imageflow_context_has_error(ctx):
        errors += 1
        eb = C.create_string_buffer(2048); w = C.c_size_t(0)
        L.imageflow_context_error_write_to_buffer(ctx, C.addressof(eb), 2048, C.byref(w))
        if first_err is None:
            first_err = eb.raw[:w.value].decode("utf-8", "replace")
    else:
        ok += 1
    if not L.imageflow_context_begin_terminate(ctx):
        begin_term_false += 1
    L.imageflow_context_destroy(ctx)
    if i in (0, 9, 99, 999) or i == n - 1:
        print(f"iter {i+1}: rss={rss_kb()} kB (start {rss0} kB) errors={errors} ok={ok}", flush=True)
print("begin_terminate returned false:", begin_term_false)
print("first error:", (first_err or "")[:200].replace("\n", " | "))
