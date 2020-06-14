# Using a JSON Graph

The following generates 4 sizes of images in a single job. Much execution time is saved
 because the image is not re-decoded for each output. 
 
Still, it is best to use a fluent API to help build JSON graphs, as it can be error prone. 

```json
{
  "io": [
    {
      "io_id": 0,
      "direction": "in",
      "io": "placeholder"
    },
    {
      "io_id": 1,
      "direction": "out",
      "io": "placeholder"
    },
    {
      "io_id": 2,
      "direction": "out",
      "io": "placeholder"
    },
    {
      "io_id": 3,
      "direction": "out",
      "io": "placeholder"
    },
    {
      "io_id": 4,
      "direction": "out",
      "io": "placeholder"
    }
  ],
  "framewise": {
    "graph": {
      "nodes": {
        "0": {
          "decode": {
            "io_id": 0
          }
        },
        "1": {
          "constrain": {
            "mode": "within",
            "w": 1600
          }
        },
        "2": {
          "constrain": {
            "mode": "within",
            "w": 1200
          }
        },
        "3": {
          "constrain": {
            "mode": "within",
            "w": 800
          }
        },
        "4": {
          "constrain": {
            "mode": "within",
            "w": 400
          }
        },
        "5": {
          "encode": {
            "io_id": 1,
            "preset": {
              "mozjpeg": {
                "quality": 90
              }
            }
          }
        },
        "6": {
          "encode": {
            "io_id": 2,
            "preset": {
              "mozjpeg": {
                "quality": 90
              }
            }
          }
        },
        "7": {
          "encode": {
            "io_id": 3,
            "preset": {
              "mozjpeg": {
                "quality": 90
              }
            }
          }
        }
        "8": {
          "encode": {
            "io_id": 4,
            "preset": {
              "mozjpeg": {
                "quality": 90
              }
            }
          }
        },
      },
      "edges": [
        {
          "from": 4,
          "to": 8,
          "kind": "input"
        },
        {
          "from": 2,
          "to": 4,
          "kind": "input"
        },
        {
          "from": 1,
          "to": 2,
          "kind": "input"
        },
        {
          "from": 0,
          "to": 1,
          "kind": "input"
        },
        {
          "from": 3,
          "to": 7,
          "kind": "input"
        },
        {
          "from": 1,
          "to": 3,
          "kind": "input"
        },
        {
          "from": 2,
          "to": 6,
          "kind": "input"
        },
        {
          "from": 1,
          "to": 5,
          "kind": "input"
        }
      ]
    }
  }
}
```