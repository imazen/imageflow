using System;
using System.Diagnostics;
using Xunit;
using Imageflow;
using System.Dynamic;
using System.IO;
using System.Text;
using Imageflow.Bindings;
using Xunit.Abstractions;
namespace Imageflow.Test
{
    public class TestContext
    {
        private readonly ITestOutputHelper output;

        public TestContext(ITestOutputHelper output)
        {
            this.output = output;
        }

        [Fact]
        public void TestCreateDestroyContext()
        {
            using (var c = new JobContext())
            {
                c.AssertReady();
            }
        }
        
         [Fact]
        public void TestGetImageInfo()
        {
            using (var c = new JobContext())
            {
                c.AddInputBytesPinned(0,
                    Convert.FromBase64String(
                        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg=="));
                
                var response = c.SendMessage("v0.1/get_image_info", new {io_id = 0});

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
            using (var c = new JobContext())
            {
                c.AddInputBytesPinned(0,
                    Convert.FromBase64String(
                        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg=="));
                
                c.AddOutputBuffer(1);
                
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
                
                var response = c.SendMessage("v0.1/execute", message);

                var data = response.DeserializeDynamic();

                output.WriteLine(response.GetString());

                Assert.Equal(200, (int)data.code);
                Assert.Equal(true, (bool)data.success);
            }
        }
        
        
        [Fact]
        public void TestIr4Execute()
        {
            using (var c = new JobContext())
            {
                c.AddInputBytesPinned(0,
                    Convert.FromBase64String(
                        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg=="));
                c.AddOutputBuffer(1);
                var response = c.ExecuteImageResizer4CommandString(0, 1, "w=200&h=200&scale=both&format=jpg");

                var data = response.DeserializeDynamic();

                output.WriteLine(response.GetString());

                Assert.Equal(200, (int)data.code);
                Assert.Equal(true, (bool)data.success);
            }
        }
        
        [Fact]
        public void TestIr4Build()
        {
            using (var c = new JobContext())
            {
                var message = new
                {
                    io = new object[]
                    {
                        new {
                            direction = "in",
                            io_id = 0,
                            io = new
                            {
                                base_64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg=="
                            }
                        },
                        new {
                            direction = "out",
                            io_id = 1,
                            io = "output_base_64"
                        }
                    },
                    framewise = new
                    {
                        steps = new object[]
                        {
                            new
                            {
                                command_string = new
                                {
                                    kind = "ir4",
                                    value = "w=200&h=200&scale=both&format=jpg",
                                    decode = 0,
                                    encode = 1
                                }
                            }
                        }
                    }
                };

               var response =  c.SendMessage("v0.1/build", message);

                var data = response.DeserializeDynamic();

                output.WriteLine(response.GetString());

                Assert.Equal(200, (int)data.code);
                Assert.Equal(true, (bool)data.success);
            }
        }
        
//        [Fact]
//        public void TestFileIo()
//        {
//            string from = null;
//            string to = null;
//            try
//            {
//                from = Path.GetTempFileName();
//                to = Path.GetTempFileName();
//                File.WriteAllBytes(from,  Convert.FromBase64String(
//                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg=="));
//
//                using (var c = new JobContext())
//                {
//
//                    c.AddInputFile(0,from);
//                    c.AddOutputFile(1, to);
//                    var response = c.ExecuteImageResizer4CommandString(0, 1, "w=200&h=200&scale=both&format=jpg");
//
//                    var data = response.DeserializeDynamic();
//
//                    output.WriteLine(response.GetString());
//
//                    Assert.Equal(200, (int) data.code);
//                    Assert.Equal(true, (bool) data.success);
//                    Assert.True(File.ReadAllBytes(to).Length > 0);
//                }
//            }
//            finally
//            {
//                if (from != null) File.Delete(from);
//                if (to != null) File.Delete(to);
//            }
//            
//        }
    }
}
