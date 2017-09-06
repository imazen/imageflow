using System;
using System.Diagnostics;
using Xunit;
using Imageflow;
using System.Dynamic;
using System.IO;
using System.Text;
using Imageflow.Native;
using Xunit.Abstractions;

namespace Imageflow.Test
{
    public class TestApi
    {
        private readonly ITestOutputHelper output;

        public TestApi(ITestOutputHelper output)
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
    }
}