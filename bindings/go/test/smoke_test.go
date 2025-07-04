package imageflow_go_client

/*
#cgo LDFLAGS: -L. -limageflow
#include <imageflow.h>
#include <stdlib.h>
*/
import "C"
import (
	"encoding/json"
	"fmt"
	"testing"
	"unsafe"
)

const IMAGEFLOW_ABI_VER_MAJOR = 0
const IMAGEFLOW_ABI_VER_MINOR = 0

func TestSmoke(t *testing.T) {
	// Step 1: Create a context
	ctx := C.imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR)
	if ctx == nil {
		t.Fatal("Failed to create Imageflow context")
	}
	defer C.imageflow_context_destroy(ctx)

	fmt.Println("Successfully created and destroyed an Imageflow context.")

	// Step 2: Call the /v1/version endpoint
	method := C.CString("/v1/version")
	defer C.free(unsafe.Pointer(method))

	jsonRequest := C.CString("{}")
	defer C.free(unsafe.Pointer(jsonRequest))

	jsonResponse := C.imageflow_context_send_json(ctx, method, (*C.uint8_t)(unsafe.Pointer(jsonRequest)), C.size_t(len("{}")))

	if C.imageflow_context_has_error(ctx) != 0 {
		buffer := make([]byte, 4096)
		C.imageflow_context_error_write_to_buffer(ctx, (*C.char)(unsafe.Pointer(&buffer[0])), C.size_t(len(buffer)))
		t.Fatalf("Imageflow context error: %s", string(buffer))
	}

	if jsonResponse == nil {
		t.Fatal("Failed to get a JSON response from /v1/version")
	}
	defer C.imageflow_json_response_destroy(ctx, jsonResponse)

	// Step 3: Read and verify the response
	var status C.int64_t
	var bufferPtr *C.uint8_t
	var bufferLen C.size_t

	if C.imageflow_json_response_read(ctx, jsonResponse, &status, &bufferPtr, &bufferLen) == 0 {
		t.Fatal("Failed to read JSON response")
	}

	if status != 200 {
		t.Fatalf("Expected HTTP status 200, but got %d", status)
	}

	responseBytes := C.GoBytes(unsafe.Pointer(bufferPtr), C.int(bufferLen))
	var responseData map[string]interface{}
	if err := json.Unmarshal(responseBytes, &responseData); err != nil {
		t.Fatalf("Failed to unmarshal JSON response: %v", err)
	}

	if _, ok := responseData["version_info"]; !ok {
		t.Fatal("Response did not contain 'version_info'")
	}

	fmt.Println("Go smoke test passed")
}
