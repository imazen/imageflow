using System;
using System.Diagnostics;
using Xunit;
using Imageflow;
using System.Dynamic;
using System.Text;
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
            using (var c = new Context())
            {
                c.AssertReady();
            }
        }
    }
}
