using System;
using System.Diagnostics;
using Xunit;
using Imageflow;
using System.Dynamic;
using System.Text;
using Xunit.Abstractions;

namespace Imageflow.Test
{
    public class TestJob
    {
        private readonly ITestOutputHelper output;

        public TestJob(ITestOutputHelper output)
        {
            this.output = output;
        }
        
        [Fact]
        public void TestCreateDestroyJob()
        {
            using (var c = new Context())
            using (var j = new Job(c))
            {
                c.AssertReady();
            }
        }

        [Fact]
        public void TestGetImageInfo()
        {
            using (var c = new Context())
            {
                var j = new Job(c);
                var inBuf = JobIo.PinManagedBytes(c,
                    Convert.FromBase64String(
                        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg=="));
                j.AddIo(inBuf, 0, Native.Direction.In);

                var response = j.SendMessage("v0.1/get_image_info", new {io_id = 0});

                var data = response.DeserializeDynamic();

                output.WriteLine(response.GetString());


                Assert.Equal(200, (int)data.code );
                Assert.Equal(true, (bool)data.success);
                Assert.Equal(1, (int)data.data.image_info.image_width);
                Assert.Equal(1, (int)data.data.image_info.image_height);
                Assert.Equal("image/png", (string)data.data.image_info.preferred_mime_type);
                Assert.Equal("png", (string)data.data.image_info.preferred_extension);
            }
        }
        
        [Fact]
        public void TestExecute()
        {
            using (var c = new Context())
            {
                var j = new Job(c);
                var inBuf = JobIo.PinManagedBytes(c,
                    Convert.FromBase64String(
                        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg=="));
                j.AddIo(inBuf, 0, Native.Direction.In);

                var outBuf = JobIo.OutputBuffer(c);
                j.AddIo(outBuf, 1, Native.Direction.Out);

                var message = new
                {
                    framewise = new
                    {
                        steps = new object[]
                        {
                            new
                            {
                                decode = new
                                {
                                    io_id = 0
                                }
                            },
                            "flip_v",
                            new
                            {
                                encode = new
                                {
                                    io_id=1,
                                    preset = new
                                    {
                                        libjpegturbo = new
                                        {
                                            quality = 90
                                        }
                                    }
                                }
                            }
                        }
                    }
                };
                
                var response = j.SendMessage("v0.1/execute", message);

                var data = response.DeserializeDynamic();

                output.WriteLine(response.GetString());

                Assert.Equal(200, (int)data.code);
                Assert.Equal(true, (bool)data.success);
            }
        }
        
    }
}
