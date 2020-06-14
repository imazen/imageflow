# Command String Command

It's possible to execute a querystring command within a JSON file. 

This is commonly used by `libimageflow` bindings to execute querystring commands. 

When specifying a `decode` `io_id`, no `decode` node is needed. 
When specifying an `encode` `io_id`, no `encode` node is needed.

When specifying both, no other nodes are needed in the job. This is the preferred method of use, as
decode hints will be optimized and encoder commands will be honored. 

```json
{
  "command_string": {
    "kind": "ir4",
    "value": "width=100&height=100&mode=max",
    "decode": 0,
    "encode": 1  
  }
}
```
