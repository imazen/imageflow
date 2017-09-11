using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Imageflow.Test.DotNetFull
{
    [TestClass]
    public class TestDotNetClassicLibraryLoading
    {
        [TestMethod]
        public void TestAccessAbi()
        {
            using (var j = new Imageflow.Bindings.JobContext()) { }
        }
    }
}
